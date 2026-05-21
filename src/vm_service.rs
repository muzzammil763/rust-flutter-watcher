use anyhow::{Context, Result};
use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;
use tokio_tungstenite::{connect_async, tungstenite::Message};
use tracing::{debug, info, warn};

pub struct VmServiceClient {
    http_url: String,
}

impl VmServiceClient {
    pub fn new(url: &str) -> Self {
        let http_url = url.trim_end_matches('/').to_string() + "/";
        Self { http_url }
    }

    pub async fn hot_reload(&self) -> Result<()> {
        let ws_url = resolve_ws_url(&self.http_url).await?;
        debug!("Connecting to VM service WebSocket at {}", ws_url);

        let (mut ws, _) = connect_async(&ws_url)
            .await
            .context("Failed to connect to Dart VM service WebSocket")?;

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

/// Resolve the full WebSocket URL for the Dart VM service.
///
/// Dart SDK ≥ 3.x protects the service with an auth token in the path:
///   http://127.0.0.1:PORT/<token>/    ← what `flutter run` prints
///   ws://127.0.0.1:PORT/<token>/ws   ← WebSocket endpoint
///
/// Strategy (in order):
///   1. HTTP GET / → follow Location redirect header
///   2. Scan body for token-shaped path segments
///   3. Try flutter attach --machine to get the URI from daemon events
///   4. Fall back to no-auth (works with old Dart SDK)
async fn resolve_ws_url(http_url: &str) -> Result<String> {
    let url = http_url.trim_end_matches('/');
    let without_scheme = url.strip_prefix("http://").unwrap_or(url);

    // If the URL already contains an auth token in the path (e.g. /V2x2YUXCXmc=/),
    // convert directly — do NOT re-probe, which would strip the token.
    let path = without_scheme.find('/').map(|i| &without_scheme[i..]).unwrap_or("/");
    if path.len() > 1 {
        let ws = format!("{}/ws", url.replace("http://", "ws://"));
        info!("VM service URL (auth token present): {}", ws);
        return Ok(ws);
    }

    let host_port = without_scheme.split('/').next().unwrap_or(without_scheme);

    // ── 1. HTTP GET / ────────────────────────────────────────────────────────
    let http_result = tokio::time::timeout(
        std::time::Duration::from_secs(5),
        probe_http(host_port),
    )
    .await;

    if let Ok(Ok(response)) = http_result {
        debug!("VM service HTTP response ({} bytes):\n{}", response.len(),
               &response.chars().take(800).collect::<String>());

        // 1a. Location redirect header
        for line in response.lines() {
            if line.len() > 9 && line[..9].eq_ignore_ascii_case("location:") {
                let loc = line[9..].trim();
                if !loc.is_empty() {
                    let ws = loc.replace("http://", "ws://").trim_end_matches('/').to_string() + "/ws";
                    info!("VM service auth URL (from Location redirect): {}", ws);
                    return Ok(ws);
                }
            }
        }

        // 1b. Scan body for a Dart-style auth token (/<token>/ path segment)
        if let Some(token) = extract_dart_auth_token(&response, host_port) {
            let ws = format!("ws://{}/{}/ws", host_port, token);
            info!("VM service auth URL (from response body): {}", ws);
            return Ok(ws);
        }
    }

    // ── 2. flutter attach --machine ──────────────────────────────────────────
    info!("HTTP probe found no auth token — trying `flutter attach --machine` for URI discovery (up to 20s)…");
    if let Some(ws_uri) = discover_via_flutter_attach_machine(None).await {
        info!("VM service URI via flutter attach: {}", ws_uri);
        return Ok(ws_uri);
    }

    // ── 3. Fallback (old Dart SDK, no auth) ─────────────────────────────────
    let ws = format!("ws://{}/ws", host_port);
    warn!(
        "Could not auto-discover VM service auth token. Trying without token: {}",
        ws
    );
    warn!(
        "If hot reload fails, copy the URL from your `flutter run` output\n\
         (the line: 'The Dart VM service is listening on http://…')\n\
         and run:  flutter-watcher --attach --vm-service-url=<that-url>"
    );
    Ok(ws)
}

/// Raw HTTP GET / to the VM service, returns the full response as a String.
async fn probe_http(host_port: &str) -> Result<String> {
    let mut stream = TcpStream::connect(host_port)
        .await
        .context("Cannot reach VM service over TCP")?;

    let req = format!(
        "GET / HTTP/1.1\r\nHost: {}\r\nAccept: */*\r\nConnection: close\r\n\r\n",
        host_port
    );
    stream.write_all(req.as_bytes()).await?;

    let mut response = Vec::with_capacity(4096);
    let mut chunk = [0u8; 1024];
    loop {
        match stream.read(&mut chunk).await {
            Ok(0) | Err(_) => break,
            Ok(n) => response.extend_from_slice(&chunk[..n]),
        }
        if response.len() > 65_536 {
            break;
        }
    }
    Ok(String::from_utf8_lossy(&response).into_owned())
}

/// Scan a string for a Dart VM service auth token.
/// Tokens look like: /<alphanumeric_base64>=/ (8+ chars, not an IP segment).
fn extract_dart_auth_token(text: &str, host_port: &str) -> Option<String> {
    let bytes = text.as_bytes();
    let mut i = 0;
    while i < bytes.len().saturating_sub(1) {
        if bytes[i] == b'/' {
            let start = i + 1;
            let mut end = start;
            while end < bytes.len() && is_token_char(bytes[end]) {
                end += 1;
            }
            let candidate = &text[start..end];
            if candidate.len() >= 8
                && end < bytes.len()
                && bytes[end] == b'/'
                && candidate.chars().any(|c| c.is_ascii_alphabetic())
                && !candidate.contains('.')
                && !host_port.contains(candidate) // not a port number
            {
                return Some(candidate.to_string());
            }
        }
        i += 1;
    }
    None
}

fn is_token_char(c: u8) -> bool {
    c.is_ascii_alphanumeric() || c == b'-' || c == b'_' || c == b'='
}

/// Run `flutter attach --machine` briefly and parse JSON daemon events to
/// extract the VM service WebSocket URI (contains the auth token).
async fn discover_via_flutter_attach_machine(device_id: Option<&str>) -> Option<String> {
    let mut cmd = tokio::process::Command::new("flutter");
    cmd.arg("attach")
        .arg("--machine")
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null());

    if let Some(id) = device_id {
        cmd.arg("-d").arg(id);
    }

    let mut child = cmd.spawn().ok()?;
    let stdout = child.stdout.take()?;
    let reader = BufReader::new(stdout);
    let mut lines = reader.lines();

    let result = tokio::time::timeout(
        std::time::Duration::from_secs(20),
        async {
            while let Ok(Some(line)) = lines.next_line().await {
                debug!("flutter attach --machine: {}", line);
                // Machine output is JSON arrays: [{"event":"...","params":{...}}]
                let val: Value = match serde_json::from_str(&line) {
                    Ok(v) => v,
                    Err(_) => continue,
                };
                let events = if val.is_array() {
                    val.as_array().cloned().unwrap_or_default()
                } else {
                    vec![val]
                };
                for event in events {
                    // app.debugPort carries wsUri
                    if event.get("event").and_then(|e| e.as_str()) == Some("app.debugPort") {
                        if let Some(uri) = event["params"]["wsUri"].as_str() {
                            return Some(uri.to_string());
                        }
                    }
                    // Generic scan across all params
                    if let Some(params) = event.get("params") {
                        for key in &["wsUri", "vmServiceUri", "observatoryUri", "uri"] {
                            if let Some(uri) = params.get(key).and_then(|v| v.as_str()) {
                                if uri.starts_with("ws://") || uri.starts_with("http://") {
                                    return Some(uri.to_string());
                                }
                            }
                        }
                    }
                }
            }
            None
        },
    )
    .await;

    let _ = child.kill().await;
    result.ok().flatten()
}

// ── ADB discovery ─────────────────────────────────────────────────────────────

/// Find the Dart VM service base URL via `adb forward --list`.
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
                    info!("Auto-discovered VM service base URL via ADB: {}", url);
                    return Some(url);
                }
            }
        }
    }
    None
}
