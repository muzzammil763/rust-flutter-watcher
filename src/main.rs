use anyhow::Result;
use clap::Parser;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::time::sleep;
use tracing::{error, info, warn};

mod cli;
mod config;
mod events;
mod flutter;
mod watcher;

use cli::Args;
use config::Config;
use events::FileEvent;
use flutter::FlutterProcess;
use watcher::FileWatcher;

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    // Setup tracing
    let filter = if args.verbose {
        "flutter_watcher=debug"
    } else {
        "flutter_watcher=info"
    };
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .init();

    // Load config
    let config = Arc::new(Config::load(args.config)?);
    let debounce_ms = if args.debounce != 300 {
        args.debounce
    } else {
        config.debounce_ms
    };

    let project_path = std::fs::canonicalize(&args.path)?;
    info!("Project path: {:?}", project_path);

    // Start or attach to flutter process
    let (mut flutter, child, mut ready_rx) = if args.attach {
        info!("Attach mode: connecting to already-running Flutter app");
        FlutterProcess::attach(&project_path, args.device_id.as_deref()).await?
    } else {
        FlutterProcess::spawn(&project_path).await?
    };
    let mut child = Some(child);

    // Channels
    let (event_tx, mut event_rx) = mpsc::channel::<FileEvent>(128);

    // Start file watcher
    let _watcher = FileWatcher::new(project_path.clone(), Arc::clone(&config), event_tx)?;

    // Debounce state
    let mut pending_reload = false;
    let mut last_event_time = tokio::time::Instant::now();
    let mut is_ready = false;

    loop {
        tokio::select! {
            _ = ready_rx.recv() => {
                is_ready = true;
                info!("Flutter process is ready to accept commands");
            }

            Some(event) = event_rx.recv() => {
                info!("Detected change: {:?}", event.path);
                pending_reload = true;
                last_event_time = tokio::time::Instant::now();
            }

            _ = sleep(Duration::from_millis(debounce_ms)) => {
                if pending_reload {
                    let now = tokio::time::Instant::now();
                    let elapsed = now.duration_since(last_event_time).as_millis() as u64;

                    if elapsed >= debounce_ms {
                        if is_ready {
                            pending_reload = false;
                            if let Err(e) = flutter.reload().await {
                                error!("Failed to send reload: {:?}", e);
                            }
                        } else {
                            warn!("Flutter not ready yet, holding reload until connected...");
                        }
                    }
                }
            }

            status = async {
                match child.take() {
                    Some(mut c) => c.wait().await,
                    None => std::future::pending().await,
                }
            } => {
                match status {
                    Ok(status) if status.success() => info!("Flutter process exited successfully"),
                    Ok(status) => error!("Flutter process exited with status: {}", status),
                    Err(e) => error!("Flutter process wait error: {:?}", e),
                }
                break;
            }

            Ok(()) = tokio::signal::ctrl_c() => {
                info!("Received Ctrl+C, shutting down...");
                if let Some(mut c) = child.take() {
                    let _ = c.kill().await;
                }
                break;
            }
        }
    }

    Ok(())
}
