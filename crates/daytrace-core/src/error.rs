use thiserror::Error;

#[derive(Debug, Error)]
pub enum CoreError {
    #[error("Database error: {0}")]
    Database(#[from] rusqlite::Error),
    #[error("File error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Invalid data: {0}")]
    Json(#[from] serde_json::Error),
    #[error("{0}")]
    Invalid(String),
    #[error("{0} was not found")]
    NotFound(&'static str),
    #[error("This period overlaps an existing session")]
    Overlap,
    #[error("This database was created by a newer version of DayTrace")]
    NewerSchema,
}

pub type Result<T> = std::result::Result<T, CoreError>;

pub fn invalid<T>(message: impl Into<String>) -> Result<T> {
    Err(CoreError::Invalid(message.into()))
}
