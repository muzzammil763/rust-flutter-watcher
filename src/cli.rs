use clap::Parser;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(name = "flutter-watcher")]
#[command(about = "Auto hot-reload watcher for Flutter projects")]
#[command(version = env!("CARGO_PKG_VERSION"))]
pub struct Args {
    /// Path to the Flutter project directory
    #[arg(short, long, default_value = ".")]
    pub path: PathBuf,

    /// Debounce duration in milliseconds
    #[arg(short, long, default_value_t = 300)]
    pub debounce: u64,

    /// Enable verbose logging
    #[arg(short, long)]
    pub verbose: bool,

    /// Path to a custom config file
    #[arg(short, long)]
    pub config: Option<PathBuf>,

    /// Attach to an already-running Flutter app instead of starting a new one
    #[arg(short, long)]
    pub attach: bool,

    /// Device ID to attach to (optional, used with --attach)
    #[arg(long)]
    pub device_id: Option<String>,

    /// VM service URL to connect to (optional, used with --attach).
    /// If not provided, flutter-watcher tries ADB auto-discovery then mDNS.
    /// Example: --vm-service-url=http://127.0.0.1:12345/
    #[arg(long)]
    pub vm_service_url: Option<String>,
}
