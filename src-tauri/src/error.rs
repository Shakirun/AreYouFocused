use serde::Serializer;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("database: {0}")]
    Db(#[from] rusqlite::Error),
    #[error("capture text must not be empty")]
    EmptyCapture,
    #[error("ping interval: {0}")]
    InvalidPingBounds(String),
    #[error("notification: {0}")]
    Notify(String),
}

impl serde::Serialize for AppError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.to_string().as_str())
    }
}
