use thiserror::Error;

#[derive(Debug, Error)]
pub enum CoreError {
    #[error("database error: {0}")]
    Database(#[from] rusqlite::Error),
    #[error("file error: {0}")]
    Io(#[from] std::io::Error),
    #[error("invalid data: {0}")]
    Json(#[from] serde_json::Error),
    #[error("{0}")]
    Invalid(String),
    #[error("{0} was not found")]
    NotFound(&'static str),
    #[error("this period overlaps an existing session")]
    Overlap,
    #[error("the database was created by a newer DayTrace version")]
    NewerSchema,
}

pub type Result<T> = std::result::Result<T, CoreError>;

pub fn invalid<T>(message: impl Into<String>) -> Result<T> {
    Err(CoreError::Invalid(message.into()))
}
