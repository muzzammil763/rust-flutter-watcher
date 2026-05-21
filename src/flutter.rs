use anyhow::Result;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, Command};
use tokio::sync::mpsc;
use tracing::info;

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

    fn from_child(mut child: Child) -> Result<(Self, Child, mpsc::Receiver<()>)> {
        let stdin = child.stdin.take().expect("Failed to capture stdin");
        let stdout = child.stdout.take().expect("Failed to capture stdout");
        let stderr = child.stderr.take().expect("Failed to capture stderr");

        let (ready_tx, ready_rx) = mpsc::channel(1);
        let ready_tx2 = ready_tx.clone();
        tokio::spawn(stdout_reader(stdout, ready_tx));
        tokio::spawn(stderr_reader(stderr, ready_tx2));

        Ok((Self { stdin }, child, ready_rx))
    }

    pub async fn reload(&mut self) -> Result<()> {
        info!("Sending hot reload command to Flutter");
        self.stdin.write_all(b"r\n").await?;
        self.stdin.flush().await?;
        Ok(())
    }
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
