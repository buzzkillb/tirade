use anyhow::{Result, anyhow};
use std::env;

#[derive(Debug, Clone)]
pub struct Config {
    pub trading_pair: String,
    pub database_url: String,
    pub rsi_fast_period: usize,
    pub rsi_slow_period: usize,
    pub sma_short_period: usize,
    pub sma_long_period: usize,
    pub volatility_window: usize,
    pub min_confidence_threshold: f64,
    pub price_change_threshold: f64,
    pub wallet_private_keys: Vec<String>,
    pub wallet_names: Vec<String>,
}

impl Config {
    pub fn from_env() -> Result<Self> {
        // Parse wallet private keys from environment
        let wallet_private_keys = if let Ok(keys_json) = env::var("SOLANA_PRIVATE_KEYS") {
            // Parse JSON array format: ["key1", "key2", "key3"]
            serde_json::from_str::<Vec<String>>(&keys_json)
                .map_err(|e| anyhow!("Invalid SOLANA_PRIVATE_KEYS JSON format: {}", e))?
        } else if let Ok(single_key) = env::var("SOLANA_PRIVATE_KEY") {
            // Fallback to single key for backward compatibility
            vec![single_key]
        } else {
            return Err(anyhow!("Either SOLANA_PRIVATE_KEYS or SOLANA_PRIVATE_KEY must be set"));
        };

        // Parse wallet names (optional)
        let wallet_names = if let Ok(names_json) = env::var("WALLET_NAMES") {
            serde_json::from_str::<Vec<String>>(&names_json)
                .map_err(|e| anyhow!("Invalid WALLET_NAMES JSON format: {}", e))?
        } else {
            // Generate default names if not provided
            (0..wallet_private_keys.len())
                .map(|i| format!("Wallet_{}", i + 1))
                .collect()
        };

        // Validate that we have the same number of keys and names
        if wallet_names.len() != wallet_private_keys.len() {
            return Err(anyhow!("Number of wallet names ({}) must match number of private keys ({})", 
                              wallet_names.len(), wallet_private_keys.len()));
        }

        Ok(Config {
            trading_pair: env::var("TRADING_PAIR")
                .unwrap_or_else(|_| "SOL/USDC".to_string()),
            database_url: env::var("DATABASE_URL")
                .unwrap_or_else(|_| "http://localhost:8080".to_string()),
            rsi_fast_period: env::var("RSI_FAST_PERIOD")
                .unwrap_or_else(|_| "14".to_string())  // Changed from 7 to 14 for consistency
                .parse()
                .map_err(|_| anyhow!("Invalid RSI_FAST_PERIOD"))?,
            rsi_slow_period: env::var("RSI_SLOW_PERIOD")
                .unwrap_or_else(|_| "21".to_string())
                .parse()
                .map_err(|_| anyhow!("Invalid RSI_SLOW_PERIOD"))?,
            sma_short_period: env::var("SMA_SHORT_PERIOD")
                .unwrap_or_else(|_| "20".to_string())
                .parse()
                .map_err(|_| anyhow!("Invalid SMA_SHORT_PERIOD"))?,
            sma_long_period: env::var("SMA_LONG_PERIOD")
                .unwrap_or_else(|_| "50".to_string())
                .parse()
                .map_err(|_| anyhow!("Invalid SMA_LONG_PERIOD"))?,
            volatility_window: env::var("VOLATILITY_WINDOW")
                .unwrap_or_else(|_| "20".to_string())
                .parse()
                .map_err(|_| anyhow!("Invalid VOLATILITY_WINDOW"))?,
            price_change_threshold: env::var("PRICE_CHANGE_THRESHOLD")
                .unwrap_or_else(|_| "0.01".to_string()) // 1%
                .parse()
                .map_err(|_| anyhow!("Invalid PRICE_CHANGE_THRESHOLD"))?,
            min_confidence_threshold: env::var("MIN_CONFIDENCE_THRESHOLD")
                .unwrap_or_else(|_| "0.5".to_string()) // 50%
                .parse()
                .map_err(|_| anyhow!("Invalid MIN_CONFIDENCE_THRESHOLD"))?,
            wallet_private_keys,
            wallet_names,
        })
    }

    pub fn get_wallet_count(&self) -> usize {
        self.wallet_private_keys.len()
    }

    pub fn get_wallet_key(&self, index: usize) -> Option<&String> {
        self.wallet_private_keys.get(index)
    }

    pub fn get_wallet_name(&self, index: usize) -> Option<&String> {
        self.wallet_names.get(index)
    }
} 