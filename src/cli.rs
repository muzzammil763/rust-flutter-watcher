use clap::Parser;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(name = "flutter-watcher")]
#[command(about = "Auto hot-reload watcher for Flutter projects")]
#[command(version = "0.1.0")]
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
}
