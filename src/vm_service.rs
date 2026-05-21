use anyhow::{Context, Result};
use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio_tungstenite::{connect_async, tungstenite::Message};
use tracing::{debug, info, warn};

pub struct VmServiceClient {
    http_url: String,
}

impl VmServiceClient {
    /// Accepts an http:// VM service URL (with or without trailing slash / auth token).
    pub fn new(url: &str) -> Self {
        let http_url = url.trim_end_matches('/').to_string() + "/";
        Self { http_url }
    }

    pub async fn hot_reload(&self) -> Result<()> {
        // Resolve the real WebSocket URL (follows the auth-token redirect).
        let ws_url = resolve_ws_url(&self.http_url).await?;
        debug!("Connecting to VM service at {}", ws_url);

        let (mut ws, _) = connect_async(&ws_url)
            .await
            .context("Failed to connect to Dart VM service WebSocket")?;

        // Get isolate list
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
            let req = json!({
                "jsonrpc": "2.0",
                "method": "ext.flutter.hotReload",
                "params": {"isolateId": id},
                "id": 2
            })
            .to_string();
            ws.send(Message::Text(req.into())).await?;

            if let Some(Ok(resp)) = ws.next().await {
                debug!("Hot reload response: {}", resp);
            }

            break;
        }

        let _ = ws.close(None).await;
        Ok(())
    }
}

/// Resolve the real WebSocket URL for the Dart VM service.
///
/// Recent Dart SDK versions protect the VM service with an auth token:
///   http://127.0.0.1:PORT/<token>/
/// They return an HTTP 301 redirect from `/` to `/<token>/`.
/// Without following that redirect, the WebSocket handshake fails.
async fn resolve_ws_url(http_url: &str) -> Result<String> {
    // Extract "host:port" for the TCP connection
    let without_scheme = http_url
        .strip_prefix("http://")
        .unwrap_or(http_url);
    let host_port = without_scheme
        .split('/')
        .next()
        .unwrap_or(without_scheme);

    let mut stream = TcpStream::connect(host_port)
        .await
        .context("Cannot reach VM service — is Flutter running on the device?")?;

    // HTTP/1.0 so the server closes the connection when done
    let req = format!(
        "GET / HTTP/1.0\r\nHost: {}\r\nConnection: close\r\n\r\n",
        host_port
    );
    stream.write_all(req.as_bytes()).await?;

    let mut response = Vec::with_capacity(2048);
    let mut chunk = [0u8; 1024];
    loop {
        match stream.read(&mut chunk).await {
            Ok(0) | Err(_) => break,
            Ok(n) => response.extend_from_slice(&chunk[..n]),
        }
        if response.len() > 32_768 {
            break;
        }
    }

    let text = String::from_utf8_lossy(&response);

    // Look for an HTTP redirect (Location header)
    for line in text.lines() {
        if line.to_lowercase().starts_with("location:") {
            let location = line[9..].trim();
            // e.g. http://127.0.0.1:PORT/<token>/ → ws://127.0.0.1:PORT/<token>/ws
            let ws = location
                .replace("http://", "ws://")
                .trim_end_matches('/')
                .to_string()
                + "/ws";
            info!("VM service auth URL: {}", ws);
            return Ok(ws);
        }
    }

    // No redirect — old Dart SDK without auth token
    let base = http_url.trim_end_matches('/');
    let ws = base.replace("http://", "ws://") + "/ws";
    info!("VM service URL (no auth): {}", ws);
    Ok(ws)
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
