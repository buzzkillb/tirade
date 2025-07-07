use axum::http::StatusCode;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum DatabaseServiceError {
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),
    
    #[error("Configuration error: {0}")]
    Config(String),
    
    #[error("Validation error: {0}")]
    Validation(String),
    
    #[error("Not found: {0}")]
    NotFound(String),
    
    #[error("Internal server error: {0}")]
    Internal(String),
    
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, DatabaseServiceError>;

impl From<DatabaseServiceError> for StatusCode {
    fn from(err: DatabaseServiceError) -> Self {
        match err {
            DatabaseServiceError::Database(_) => StatusCode::INTERNAL_SERVER_ERROR,
            DatabaseServiceError::Config(_) => StatusCode::INTERNAL_SERVER_ERROR,
            DatabaseServiceError::Validation(_) => StatusCode::BAD_REQUEST,
            DatabaseServiceError::NotFound(_) => StatusCode::NOT_FOUND,
            DatabaseServiceError::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
            DatabaseServiceError::Io(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
} 