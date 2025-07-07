mod api;
mod config;
mod error;
mod price_feed;

use crate::config::Config;
use crate::error::Result;
use crate::price_feed::PriceFeedService;
use dotenv::dotenv;
use tracing_subscriber;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt::init();
    
    // Load environment variables
    dotenv().ok();
    
    // Load configuration
    let config = Config::from_env()?;
    
    // Create and run the price feed service
    let service = PriceFeedService::new(config);
    service.run().await?;
    
    Ok(())
}
