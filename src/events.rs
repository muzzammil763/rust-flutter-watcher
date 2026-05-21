use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct FileEvent {
    pub path: PathBuf,
    pub kind: EventKind,
}

#[derive(Debug, Clone)]
pub enum EventKind {
    Changed,
    Created,
    Removed,
}

impl FileEvent {
    pub fn new(path: PathBuf, kind: EventKind) -> Self {
        Self { path, kind }
    }
}
