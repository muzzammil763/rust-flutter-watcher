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
mod vm_service;
mod watcher;

use cli::Args;
use config::Config;
use events::FileEvent;
use flutter::FlutterProcess;
use vm_service::{VmServiceClient, discover_vm_service_url};
use watcher::FileWatcher;

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    let filter = if args.verbose {
        "flutter_watcher=debug"
    } else {
        "flutter_watcher=info"
    };
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .init();

    let config = Arc::new(Config::load(args.config)?);
    let debounce_ms = if args.debounce != 300 {
        args.debounce
    } else {
        config.debounce_ms
    };

    let project_path = std::fs::canonicalize(&args.path)?;
    info!("Project path: {:?}", project_path);

    let (event_tx, mut event_rx) = mpsc::channel::<FileEvent>(128);
    let _watcher = FileWatcher::new(project_path.clone(), Arc::clone(&config), event_tx)?;

    if args.attach {
        // ── Attach mode ───────────────────────────────────────────────────────
        // Directly use the Dart VM service for hot reload instead of spawning
        // `flutter attach`. This bypasses mDNS discovery and stdin-tty issues.
        let vm_url = match args.vm_service_url.clone() {
            Some(url) => {
                info!("Using provided VM service URL: {}", url);
                url
            }
            None => {
                match discover_vm_service_url().await {
                    Some(url) => url,
                    None => {
                        error!(
                            "Could not auto-discover VM service via ADB.\n\
                             Make sure your Flutter app is running on the device, then retry.\n\
                             Or specify it manually: flutter-watcher --attach --vm-service-url=http://127.0.0.1:PORT/"
                        );
                        return Ok(());
                    }
                }
            }
        };

        let client = VmServiceClient::new(&vm_url);
        info!("Attach mode ready — watching for file changes");

        let mut pending_reload = false;
        let mut last_event_time = tokio::time::Instant::now();

        loop {
            tokio::select! {
                Some(event) = event_rx.recv() => {
                    info!("Detected change: {:?}", event.path);
                    pending_reload = true;
                    last_event_time = tokio::time::Instant::now();
                }

                _ = sleep(Duration::from_millis(debounce_ms)) => {
                    if pending_reload {
                        let elapsed = last_event_time.elapsed().as_millis() as u64;
                        if elapsed >= debounce_ms {
                            pending_reload = false;
                            if let Err(e) = client.hot_reload().await {
                                error!("Hot reload failed: {:?}", e);
                            }
                        }
                    }
                }

                Ok(()) = tokio::signal::ctrl_c() => {
                    info!("Received Ctrl+C, shutting down...");
                    break;
                }
            }
        }
    } else {
        // ── Standard mode ─────────────────────────────────────────────────────
        // Spawn our own `flutter run` and pipe `r\n` to its stdin on change.
        let (mut flutter, child, mut ready_rx) = FlutterProcess::spawn(&project_path).await?;
        let mut child = Some(child);

        let mut pending_reload = false;
        let mut last_event_time = tokio::time::Instant::now();
        let mut last_not_ready_warn = tokio::time::Instant::now();
        let mut is_ready = false;
        let mut ready_done = false;

        loop {
            tokio::select! {
                result = async {
                    if ready_done {
                        std::future::pending::<Option<()>>().await
                    } else {
                        ready_rx.recv().await
                    }
                } => {
                    match result {
                        Some(()) if !is_ready => {
                            is_ready = true;
                            ready_done = true;
                            info!("Flutter process is ready to accept commands");
                        }
                        None => {
                            ready_done = true; // channel closed, stop polling
                        }
                        _ => {}
                    }
                }

                Some(event) = event_rx.recv() => {
                    info!("Detected change: {:?}", event.path);
                    pending_reload = true;
                    last_event_time = tokio::time::Instant::now();
                }

                _ = sleep(Duration::from_millis(debounce_ms)) => {
                    if pending_reload {
                        let elapsed = last_event_time.elapsed().as_millis() as u64;
                        if elapsed >= debounce_ms {
                            if is_ready {
                                pending_reload = false;
                                if let Err(e) = flutter.reload().await {
                                    error!("Failed to send reload: {:?}", e);
                                }
                            } else {
                                let now = tokio::time::Instant::now();
                                if now.duration_since(last_not_ready_warn).as_secs() >= 5 {
                                    warn!("Waiting for Flutter to be ready...");
                                    last_not_ready_warn = now;
                                }
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
                        Ok(s) if s.success() => info!("Flutter process exited successfully"),
                        Ok(s) => error!("Flutter process exited with status: {}", s),
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
    }

    Ok(())
}
