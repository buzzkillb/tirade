mod config;
mod models;
mod strategy;
mod trading_engine;
mod trading_executor;

use anyhow::Result;
use tracing::info;
use tracing_subscriber;

use crate::config::Config;
use crate::trading_engine::TradingEngine;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt::init();
    
    // Load configuration from project root .env file
    let project_root = std::env::current_dir()?.join("..");
    let env_path = project_root.join(".env");
    info!("Looking for .env file at: {:?}", env_path);
    dotenv::from_path(&env_path).ok();
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