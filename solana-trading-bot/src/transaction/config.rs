use solana_sdk::{
    signature::Keypair,
};
use crate::transaction::error::TransactionError;

#[derive(Debug, Clone)]
pub struct Config {
    pub rpc_url: String,
    pub private_key: String,
    pub usdc_mint: String,
    pub sol_mint: String,
    pub jupiter_base_url: String,
}

impl Config {
    pub fn load() -> Result<Self, TransactionError> {
        let rpc_url = std::env::var("SOLANA_RPC_URL")
            .unwrap_or_else(|_| "https://api.mainnet-beta.solana.com".to_string());
        
        let private_key = std::env::var("SOLANA_PRIVATE_KEY")
            .map_err(|_| TransactionError::Config("SOLANA_PRIVATE_KEY not found in .env file".to_string()))?;
        
        let usdc_mint = std::env::var("USDC_MINT")
            .unwrap_or_else(|_| "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v".to_string());
        
        let sol_mint = std::env::var("SOL_MINT")
            .unwrap_or_else(|_| "So11111111111111111111111111111111111111112".to_string());
        
        Ok(Config {
            rpc_url,
            private_key,
            usdc_mint,
            sol_mint,
            jupiter_base_url: "https://quote-api.jup.ag/v6".to_string(),
        })
    }
}

pub fn create_wallet_from_private_key(private_key: &str) -> Result<Keypair, TransactionError> {
    let private_key = private_key.trim();
    let keypair_bytes = if private_key.starts_with('[') {
        serde_json::from_str::<Vec<u8>>(private_key)?
    } else {
        bs58::decode(private_key).into_vec()?
    };
    
    if keypair_bytes.len() != 64 {
        return Err(TransactionError::Config("Invalid private key length".to_string()));
    }
    
    let keypair = Keypair::from_bytes(&keypair_bytes)
        .map_err(|e| TransactionError::Serialization(e.to_string()))?;
    Ok(keypair)
} 