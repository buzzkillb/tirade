use anyhow::{Result, anyhow};
use std::env;

#[derive(Debug, Clone)]
pub struct Config {
    pub database_url: String,
    pub trading_pair: String,
    pub min_data_points: usize,
    pub check_interval_secs: u64,
    pub rsi_fast_period: usize,
    pub rsi_slow_period: usize,
    pub sma_short_period: usize,
    pub sma_long_period: usize,
    pub volatility_window: usize,
    pub price_change_threshold: f64,
    pub stop_loss_threshold: f64,
    pub take_profit_threshold: f64,
}

impl Config {
    pub fn from_env() -> Result<Self> {
        Ok(Self {
            database_url: env::var("PRICE_FEED_DATABASE_URL")
                .unwrap_or_else(|_| "http://localhost:8080".to_string()),
            trading_pair: env::var("TRADING_PAIR")
                .unwrap_or_else(|_| "SOL/USDC".to_string()),
            min_data_points: env::var("MIN_DATA_POINTS")
                .unwrap_or_else(|_| "200".to_string())
                .parse()
                .map_err(|_| anyhow!("Invalid MIN_DATA_POINTS"))?,
            check_interval_secs: env::var("CHECK_INTERVAL_SECS")
                .unwrap_or_else(|_| "30".to_string())
                .parse()
                .map_err(|_| anyhow!("Invalid CHECK_INTERVAL_SECS"))?,
            rsi_fast_period: env::var("RSI_FAST_PERIOD")
                .unwrap_or_else(|_| "7".to_string())
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
                .unwrap_or_else(|_| "0.005".to_string()) // 0.5%
                .parse()
                .map_err(|_| anyhow!("Invalid PRICE_CHANGE_THRESHOLD"))?,
            stop_loss_threshold: env::var("STOP_LOSS_THRESHOLD")
                .unwrap_or_else(|_| "0.02".to_string()) // 2%
                .parse()
                .map_err(|_| anyhow!("Invalid STOP_LOSS_THRESHOLD"))?,
            take_profit_threshold: env::var("TAKE_PROFIT_THRESHOLD")
                .unwrap_or_else(|_| "0.015".to_string()) // 1.5%
                .parse()
                .map_err(|_| anyhow!("Invalid TAKE_PROFIT_THRESHOLD"))?,
        })
    }
} 