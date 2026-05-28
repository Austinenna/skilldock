use serde::{Serialize, Serializer};

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("home directory not found")]
    NoHomeDir,
    #[error("skill not found: {0}")]
    SkillNotFound(String),
    #[error("platform not found: {0}")]
    PlatformNotFound(String),
    #[error("cannot route to hub")]
    HubRoute,
    #[error("conflict: {0}")]
    Conflict(String),
    #[error("invalid name: {0}")]
    InvalidName(String),
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
    #[error("watcher: {0}")]
    Watcher(#[from] notify::Error),
    #[error("keychain: {0}")]
    Keyring(#[from] keyring::Error),
    #[error("database: {0}")]
    Database(String),
    #[error("ai: {0}")]
    Ai(String),
}

impl Serialize for AppError {
    fn serialize<S: Serializer>(&self, s: S) -> std::result::Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}

pub type Result<T> = std::result::Result<T, AppError>;
