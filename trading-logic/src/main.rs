mod config;
mod models;
mod strategy;
mod trading_engine;

use anyhow::Result;
use tracing::{info, warn, error};
use tracing_subscriber;

use crate::config::Config;
use crate::trading_engine::TradingEngine;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt::init();
    
    // Load configuration
    dotenv::dotenv().ok();
    let config = Config::from_env()?;
    
    info!("Starting Trading Logic Engine");
    info!("Database URL: {}", config.database_url);
    info!("Trading Pair: {}", config.trading_pair);
    info!("Min Data Points: {}", config.min_data_points);
    info!("Check Interval: {} seconds", config.check_interval_secs);
    
    // Create trading engine
    let mut engine = TradingEngine::new(config).await?;
    
    // Start the trading loop
    engine.run().await?;
    
    Ok(())
} 