use anyhow::Result;
use notify::{Config as NotifyConfig, Event, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};

use crate::config::Config;
use crate::events::{EventKind, FileEvent};

pub struct FileWatcher {
    #[allow(dead_code)]
    watcher: RecommendedWatcher,
}

impl FileWatcher {
    pub fn new(
        project_path: PathBuf,
        config: Arc<Config>,
        tx: mpsc::Sender<FileEvent>,
    ) -> Result<Self> {
        // Bridge: sync notify callback → async tokio task via std channel
        let (sync_tx, sync_rx) = std::sync::mpsc::channel::<FileEvent>();
        let config_for_closure = Arc::clone(&config);

        let watcher = RecommendedWatcher::new(
            move |res: Result<Event, notify::Error>| {
                match res {
                    Ok(event) => {
                        for path in &event.paths {
                            let path = path.clone();
                            let cfg = Arc::clone(&config_for_closure);

                            if cfg.should_watch(&path) {
                                let kind = match event.kind {
                                    notify::EventKind::Create(_) => EventKind::Created,
                                    notify::EventKind::Modify(_) => EventKind::Changed,
                                    notify::EventKind::Remove(_) => EventKind::Removed,
                                    _ => continue,
                                };

                                debug!("File event: {:?} at {:?}", kind, path);
                                let _ = sync_tx.send(FileEvent::new(path, kind));
                            }
                        }
                    }
                    Err(e) => {
                        error!("Watch error: {:?}", e);
                    }
                }
            },
            NotifyConfig::default(),
        )?;

        let mut fw = FileWatcher { watcher };

        for watch_path in &config.watch_paths {
            let full_path = project_path.join(watch_path);
            if full_path.exists() {
                info!("Watching path: {:?}", full_path);
                fw.watcher.watch(&full_path, RecursiveMode::Recursive)?;
            } else {
                warn!("Watch path does not exist: {:?}", full_path);
            }
        }

        // Spawn a Tokio task to drain the sync channel into the async mpsc
        tokio::spawn(async move {
            loop {
                match sync_rx.recv() {
                    Ok(event) => {
                        if tx.send(event).await.is_err() {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
        });

        Ok(fw)
    }
}
