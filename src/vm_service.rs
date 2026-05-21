use anyhow::{Context, Result};
use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use tokio_tungstenite::{connect_async, tungstenite::Message};
use tracing::{debug, info, warn};

pub struct VmServiceClient {
    ws_url: String,
}

impl VmServiceClient {
    /// Accepts an http:// or ws:// VM service URL.
    pub fn new(url: &str) -> Self {
        let ws_url = url
            .replace("http://", "ws://")
            .replace("https://", "wss://")
            .trim_end_matches('/')
            .to_string()
            + "/ws";
        Self { ws_url }
    }

    pub async fn hot_reload(&self) -> Result<()> {
        debug!("Connecting to VM service at {}", self.ws_url);

        let (mut ws, _) = connect_async(&self.ws_url)
            .await
            .context("Failed to connect to Dart VM service — is the Flutter app running?")?;

        // Ask for the list of isolates
        let get_vm = json!({"jsonrpc":"2.0","method":"getVM","id":1}).to_string();
        ws.send(Message::Text(get_vm.into())).await?;

        let msg = ws
            .next()
            .await
            .ok_or_else(|| anyhow::anyhow!("VM service closed connection"))??;

        let vm: Value = serde_json::from_str(msg.to_text()?)?;
        let isolates = match vm["result"]["isolates"].as_array() {
            Some(list) if !list.is_empty() => list.clone(),
            _ => {
                warn!("No isolates found in VM response");
                let _ = ws.close(None).await;
                return Ok(());
            }
        };

        // Trigger hot reload on the first live isolate
        for isolate in &isolates {
            let id = match isolate["id"].as_str() {
                Some(id) => id,
                None => continue,
            };

            info!("Hot reloading isolate {}", id);
            let reload_req = json!({
                "jsonrpc": "2.0",
                "method": "ext.flutter.hotReload",
                "params": {"isolateId": id},
                "id": 2
            })
            .to_string();
            ws.send(Message::Text(reload_req.into())).await?;

            if let Some(Ok(resp)) = ws.next().await {
                debug!("Hot reload response: {}", resp);
            }

            break;
        }

        let _ = ws.close(None).await;
        Ok(())
    }
}

/// Find the Dart VM service port via `adb forward --list`.
/// Flutter sets up ADB port forwarding when running on an Android device.
pub async fn discover_vm_service_url() -> Option<String> {
    let output = tokio::process::Command::new("adb")
        .args(["forward", "--list"])
        .output()
        .await
        .ok()?;

    if !output.status.success() || output.stdout.is_empty() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    for line in stdout.lines() {
        // Format: "<serial> tcp:<host_port> tcp:<device_port>"
        let mut parts = line.split_whitespace();
        parts.next(); // skip serial
        if let Some(host) = parts.next() {
            if let Some(port_str) = host.strip_prefix("tcp:") {
                if let Ok(port) = port_str.parse::<u16>() {
                    let url = format!("http://127.0.0.1:{}/", port);
                    info!("Auto-discovered VM service via ADB at {}", url);
                    return Some(url);
                }
            }
        }
    }
    None
}
