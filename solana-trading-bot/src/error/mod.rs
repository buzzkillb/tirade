use thiserror::Error;

#[derive(Error, Debug)]
pub enum TradingBotError {
    #[error("Solana client error: {0}")]
    SolanaClient(#[from] solana_client::client_error::ClientError),
    
    #[error("Solana SDK error: {0}")]
    SolanaSdk(#[from] solana_sdk::signature::SignerError),
    
    #[error("Token error: {0}")]
    Token(#[from] spl_token::error::TokenError),
    
    #[error("Configuration error: {0}")]
    Config(String),
    
    #[error("Private key error: {0}")]
    PrivateKey(String),
    
    #[error("HTTP request error: {0}")]
    Http(#[from] reqwest::Error),
    
    #[error("JSON serialization error: {0}")]
    Json(#[from] serde_json::Error),
    
    #[error("API error: {status} - {message}")]
    Api { status: u16, message: String },
    
    #[error("Validation error: {0}")]
    Validation(String),
}

pub type Result<T> = std::result::Result<T, TradingBotError>; 