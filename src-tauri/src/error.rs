use serde::{Serialize, Serializer};

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("home directory not found")]
    NoHomeDir,
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
    #[error("watcher: {0}")]
    Watcher(#[from] notify::Error),
    #[error("{0}")]
    Core(String),
}

impl From<skilldock_core::Error> for AppError {
    fn from(e: skilldock_core::Error) -> Self {
        AppError::Core(e.0)
    }
}

impl Serialize for AppError {
    fn serialize<S: Serializer>(&self, s: S) -> std::result::Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}

pub type Result<T> = std::result::Result<T, AppError>;
