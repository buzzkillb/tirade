use thiserror::Error;

#[derive(Error, Debug)]
pub enum PriceFeedError {
    #[error("HTTP request failed: {0}")]
    HttpError(#[from] reqwest::Error),
    
    #[error("JSON parsing failed: {0}")]
    JsonError(#[from] serde_json::Error),
    
    #[error("Invalid price data: {message}")]
    InvalidPriceData { message: String },
    
    #[error("API response error: {status} - {message}")]
    ApiError { status: u16, message: String },
    
    #[error("Configuration error: {0}")]
    ConfigError(String),
    
    #[error("Database error: {0}")]
    DatabaseError(String),
}

pub type Result<T> = std::result::Result<T, PriceFeedError>; 