use thiserror::Error;

#[derive(Error, Debug)]
pub enum TransactionError {
    #[error("Configuration error: {0}")]
    Config(String),
    
    #[error("Jupiter API error: {0}")]
    JupiterApi(String),
    
    #[error("Solana RPC error: {0}")]
    SolanaRpc(String),
    
    #[error("Transaction error: {0}")]
    Transaction(String),
    
    #[error("Balance error: {0}")]
    Balance(String),
    
    #[error("Serialization error: {0}")]
    Serialization(String),
    
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    
    #[error("Base64 decode error: {0}")]
    Base64(#[from] base64::DecodeError),
    
    #[error("Bincode error: {0}")]
    Bincode(#[from] bincode::Error),
    
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    
    #[error("Solana SDK error: {0}")]
    SolanaSdk(#[from] solana_sdk::signature::SignerError),
    
    #[error("Pubkey parse error: {0}")]
    PubkeyParse(#[from] solana_sdk::pubkey::ParsePubkeyError),
}

impl From<Box<dyn std::error::Error + Send + Sync>> for TransactionError {
    fn from(err: Box<dyn std::error::Error + Send + Sync>) -> Self {
        TransactionError::Transaction(err.to_string())
    }
}

impl From<bs58::decode::Error> for TransactionError {
    fn from(err: bs58::decode::Error) -> Self {
        TransactionError::Serialization(err.to_string())
    }
} 