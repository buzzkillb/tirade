use crate::error::{PriceFeedError, Result};
use std::env;

#[derive(Debug, Clone)]
pub struct Config {
    pub pyth_interval_secs: u64,
    pub jup_interval_secs: u64,
    pub pyth_feed_id: String,
    pub pyth_base_url: String,
    pub jup_base_url: String,
    pub sol_mint: String,
    pub usdc_mint: String,
    pub sol_amount: String,
    pub slippage_bps: u32,
    pub database_url: String,
}

impl Config {
    pub fn from_env() -> Result<Self> {
        let pyth_interval_secs = env::var("PYTH_INTERVAL_SECS")
            .unwrap_or_else(|_| "1".to_string())
            .parse::<u64>()
            .map_err(|_| PriceFeedError::ConfigError("Invalid PYTH_INTERVAL_SECS".to_string()))?;
            
        let jup_interval_secs = env::var("JUP_INTERVAL_SECS")
            .unwrap_or_else(|_| "10".to_string())
            .parse::<u64>()
            .map_err(|_| PriceFeedError::ConfigError("Invalid JUP_INTERVAL_SECS".to_string()))?;

        Ok(Self {
            pyth_interval_secs,
            jup_interval_secs,
            pyth_feed_id: "0xef0d8b6fda2ceba41da15d4095d1da392a0d2f8ed0c6c7bc0f4cfac8c280b56d".to_string(),
            pyth_base_url: "https://hermes.pyth.network/api".to_string(),
            jup_base_url: "https://quote-api.jup.ag/v6".to_string(),
            sol_mint: "So11111111111111111111111111111111111111112".to_string(),
            usdc_mint: "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v".to_string(),
            sol_amount: "1000000000".to_string(), // 1 SOL in lamports
            slippage_bps: 50,
            database_url: env::var("PRICE_FEED_DATABASE_URL")
                .unwrap_or_else(|_| "http://localhost:8080".to_string()),
        })
    }
} 