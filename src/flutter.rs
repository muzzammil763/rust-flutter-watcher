use anyhow::Result;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, Command};
use tokio::sync::mpsc;
use tracing::{info, warn};

pub struct FlutterProcess {
    stdin: ChildStdin,
}

impl FlutterProcess {
    pub async fn spawn(project_path: &std::path::PathBuf) -> Result<(Self, Child, mpsc::Receiver<()>)> {
        info!("Starting flutter run in {:?}", project_path);

        let child = Command::new("flutter")
            .arg("run")
            .current_dir(project_path)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()?;

        Self::from_child(child)
    }

    pub async fn attach(
        project_path: &std::path::PathBuf,
        device_id: Option<&str>,
        vm_service_url: Option<&str>,
    ) -> Result<(Self, Child, mpsc::Receiver<()>)> {
        info!("Attaching to running Flutter app in {:?}", project_path);

        let mut cmd = Command::new("flutter");
        cmd.arg("attach")
            .current_dir(project_path)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());

        if let Some(id) = device_id {
            cmd.arg("-d").arg(id);
        }

        // Resolve VM service URL: explicit flag > ADB auto-discovery > mDNS fallback
        let resolved_url = match vm_service_url {
            Some(url) => {
                info!("Using provided VM service URL: {}", url);
                Some(url.to_string())
            }
            None => {
                let discovered = discover_vm_service_url().await;
                if discovered.is_none() {
                    warn!("Could not auto-discover VM service via ADB. Falling back to mDNS discovery — this may be slow or fail on some setups. If attach hangs, try: flutter-watcher --attach --vm-service-url=<url>");
                }
                discovered
            }
        };

        if let Some(url) = resolved_url {
            info!("Connecting to VM service at {}", url);
            cmd.arg(format!("--vm-service-url={}", url));
        }

        let child = cmd.spawn()?;
        Self::from_child(child)
    }

    fn from_child(mut child: Child) -> Result<(Self, Child, mpsc::Receiver<()>)> {

        let stdin = child.stdin.take().expect("Failed to capture stdin");
        let stdout = child.stdout.take().expect("Failed to capture stdout");
        let stderr = child.stderr.take().expect("Failed to capture stderr");

        let (ready_tx, ready_rx) = mpsc::channel(1);

        let ready_tx2 = ready_tx.clone();
        tokio::spawn(stdout_reader(stdout, ready_tx));
        tokio::spawn(stderr_reader(stderr, ready_tx2));

        let process = Self { stdin };

        Ok((process, child, ready_rx))
    }

    pub async fn reload(&mut self) -> Result<()> {
        info!("Sending hot reload command to Flutter");
        self.stdin.write_all(b"r\n").await?;
        self.stdin.flush().await?;
        Ok(())
    }
}

/// Try to find the Dart VM service URL from ADB port forwarding.
/// Flutter automatically sets up ADB forwarding (tcp:PORT -> tcp:PORT) when
/// running on an Android device. Passing the URL directly to `flutter attach`
/// avoids mDNS discovery, which is unreliable on many networks.
async fn discover_vm_service_url() -> Option<String> {
    let output = Command::new("adb")
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

async fn stdout_reader(stdout: tokio::process::ChildStdout, ready_tx: mpsc::Sender<()>) {
    let reader = BufReader::new(stdout);
    let mut lines = reader.lines();

    while let Ok(Some(line)) = lines.next_line().await {
        println!("[flutter] {}", line);
        if line.contains("Flutter run key commands") {
            let _ = ready_tx.send(()).await;
        }
    }
}

async fn stderr_reader(stderr: tokio::process::ChildStderr, ready_tx: mpsc::Sender<()>) {
    let reader = BufReader::new(stderr);
    let mut lines = reader.lines();

    while let Ok(Some(line)) = lines.next_line().await {
        eprintln!("[flutter] {}", line);
        if line.contains("Flutter run key commands") {
            let _ = ready_tx.send(()).await;
        }
    }
}
