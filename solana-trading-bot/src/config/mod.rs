use crate::error::{Result, TradingBotError};
use std::env;

#[derive(Debug, Clone)]
pub struct Config {
    pub rpc_url: String,
    pub private_key: String,
    pub usdc_mint: String,
    pub database_url: Option<String>,
    pub check_interval_secs: Option<u64>,
    pub continuous_mode: bool,
}

impl Config {
    pub fn from_env() -> Result<Self> {
        let rpc_url = env::var("SOLANA_RPC_URL")
            .unwrap_or_else(|_| "https://api.mainnet-beta.solana.com".to_string());
            
        let private_key = env::var("SOLANA_PRIVATE_KEY")
            .map_err(|_| TradingBotError::Config("SOLANA_PRIVATE_KEY not found in .env file".to_string()))?;
            
        let usdc_mint = env::var("USDC_MINT")
            .unwrap_or_else(|_| "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v".to_string());
            
        let database_url = env::var("DATABASE_SERVICE_URL").ok();
        
        let check_interval_secs = env::var("CHECK_INTERVAL_SECS")
            .ok()
            .and_then(|s| s.parse::<u64>().ok());
            
        let continuous_mode = env::var("CONTINUOUS_MODE")
            .unwrap_or_else(|_| "false".to_string())
            .parse::<bool>()
            .unwrap_or(false);

        Ok(Self {
            rpc_url,
            private_key,
            usdc_mint,
            database_url,
            check_interval_secs,
            continuous_mode,
        })
    }
    
    pub fn validate(&self) -> Result<()> {
        if self.private_key.trim().is_empty() {
            return Err(TradingBotError::Validation("Private key cannot be empty".to_string()));
        }
        
        if self.rpc_url.trim().is_empty() {
            return Err(TradingBotError::Validation("RPC URL cannot be empty".to_string()));
        }
        
        Ok(())
    }
} 