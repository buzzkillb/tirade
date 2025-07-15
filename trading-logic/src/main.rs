mod config;
mod models;
mod strategy;
mod trading_engine;
mod trading_executor;
mod ml_strategy;

use anyhow::Result;
use tracing::info;
use tracing_subscriber;

use crate::config::Config;
use crate::trading_engine::TradingEngine;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt::init();
    
    // Load configuration from project root .env file (two directories up from trading-logic)
    let project_root = std::env::current_dir()?;
    let env_path = if project_root.ends_with("trading-logic") {
        project_root.join("..").join(".env")
    } else {
        project_root.join(".env")
    };
    info!("Looking for .env file at: {:?}", env_path);
    dotenv::from_path(&env_path).ok();
    let config = Config::from_env()?;
    
    info!("Trading Logic Engine Starting...");
    info!("Trading Pair: {}", config.trading_pair);
    info!("Database URL: {}", config.database_url);
    info!("RSI Fast Period: {}", config.rsi_fast_period);
    info!("RSI Slow Period: {}", config.rsi_slow_period);
    info!("SMA Short Period: {}", config.sma_short_period);
    info!("SMA Long Period: {}", config.sma_long_period);
    info!("Volatility Window: {}", config.volatility_window);
    info!("Min Confidence Threshold: {:.1}%", config.min_confidence_threshold * 100.0);
    info!("Price Change Threshold: {:.1}%", config.price_change_threshold * 100.0);
    
    // Create trading engine
    let mut engine = TradingEngine::new(config).await?;
    
    // Start the trading loop
    engine.run().await?;
    
    Ok(())
} 