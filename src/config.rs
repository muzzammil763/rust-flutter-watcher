use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default = "default_watch_paths")]
    pub watch_paths: Vec<String>,

    #[serde(default = "default_ignore_paths")]
    pub ignore_paths: Vec<String>,

    #[serde(default = "default_file_extensions")]
    pub file_extensions: Vec<String>,

    #[serde(default = "default_debounce_ms")]
    pub debounce_ms: u64,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            watch_paths: default_watch_paths(),
            ignore_paths: default_ignore_paths(),
            file_extensions: default_file_extensions(),
            debounce_ms: default_debounce_ms(),
        }
    }
}

impl Config {
    pub fn load(path: Option<PathBuf>) -> Result<Self> {
        if let Some(p) = path {
            let content = std::fs::read_to_string(p)?;
            let cfg: Config = toml::from_str(&content)?;
            return Ok(cfg);
        }

        // Try loading from current directory
        let default_file = PathBuf::from("flutter-watcher.toml");
        if default_file.exists() {
            let content = std::fs::read_to_string(default_file)?;
            let cfg: Config = toml::from_str(&content)?;
            return Ok(cfg);
        }

        Ok(Config::default())
    }

    pub fn should_watch(&self, path: &PathBuf) -> bool {
        let path_str = path.to_string_lossy();

        // Check ignore paths
        for ignore in &self.ignore_paths {
            if path_str.contains(ignore) {
                return false;
            }
        }

        // Check file extension if it's a file
        if path.is_file() {
            if let Some(ext) = path.extension() {
                let ext = ext.to_string_lossy().to_lowercase();
                if !self.file_extensions.contains(&ext) {
                    return false;
                }
            } else {
                // No extension — allow only known config files like pubspec.yaml
                let file_name = path.file_name().map(|n| n.to_string_lossy()).unwrap_or_default();
                if file_name != "pubspec.yaml" {
                    return false;
                }
            }
        }

        true
    }
}

fn default_watch_paths() -> Vec<String> {
    vec!["lib".into(), "assets".into(), "pubspec.yaml".into()]
}

fn default_ignore_paths() -> Vec<String> {
    vec![
        "build".into(),
        ".dart_tool".into(),
        ".git".into(),
        "ios/Pods".into(),
        "android/build".into(),
        "android/.gradle".into(),
    ]
}

fn default_file_extensions() -> Vec<String> {
    vec![
        "dart".into(),
        "yaml".into(),
        "yml".into(),
        "json".into(),
        "png".into(),
        "jpg".into(),
        "jpeg".into(),
        "svg".into(),
        "webp".into(),
        "gif".into(),
    ]
}

fn default_debounce_ms() -> u64 {
    300
}
