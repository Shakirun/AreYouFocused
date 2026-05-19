use serde::Serializer;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("database: {0}")]
    Db(#[from] rusqlite::Error),
    #[error("capture text must not be empty")]
    EmptyCapture,
    #[error("no previous capture to repeat")]
    NoPriorCapture,
    #[error("latest capture has no logged duration to shorten")]
    NoAdjustableDuration,
    #[error("ping interval: {0}")]
    InvalidPingBounds(String),
    #[error("planned duration must be between 1 minute and one week")]
    InvalidPlannedDuration,
    #[error("notification: {0}")]
    Notify(String),
    #[error("export: {0}")]
    Export(String),
}

impl serde::Serialize for AppError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.to_string().as_str())
    }
}
