mod config;
mod error;
mod balance_checker;
mod api;

use crate::config::Config;
use crate::error::Result;
use crate::balance_checker::Wallet;
use crate::api::DatabaseApi;
use clap::Parser;
use solana_client::rpc_client::RpcClient;
use std::time::Duration;
use tracing::{error, info, warn};

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Run in continuous mode
    #[arg(short, long)]
    continuous: bool,
    
    /// Check interval in seconds (for continuous mode)
    #[arg(short, long, default_value = "60")]
    interval: u64,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt::init();
    
    // Load environment variables
    dotenv::dotenv().ok();
    
    // Parse command line arguments
    let args = Args::parse();
    
    // Load and validate configuration
    let config = Config::from_env()?;
    config.validate()?;
    
    info!("Starting Solana Trading Bot");
    info!("RPC URL: {}", config.rpc_url);
    info!("Wallet: {}", config.private_key.chars().take(8).collect::<String>() + "...");
    info!("USDC Mint: {}", config.usdc_mint);
    
    // Create Solana client
    let client = RpcClient::new(config.rpc_url.clone());
    
    // Create wallet
    let wallet = Wallet::from_private_key(&config.private_key)?;
    info!("Wallet created successfully: {}", wallet.pubkey);
    
    // Create database API if URL is provided
    let database_api = config.database_url.as_ref().map(|url| DatabaseApi::new(url.clone()));
    
    if let Some(ref api) = database_api {
        info!("Database service URL: {}", config.database_url.as_ref().unwrap());
        
        // Check database service health
        match api.health_check().await {
            Ok(healthy) => {
                if healthy {
                    info!("Database service is healthy");
                } else {
                    warn!("Database service is not responding");
                }
            }
            Err(e) => {
                warn!("Failed to check database service health: {}", e);
            }
        }
    } else {
        info!("No database service URL provided, running in standalone mode");
    }
    
    // Determine if we should run continuously
    let continuous = args.continuous || config.continuous_mode;
    let interval = args.interval.min(config.check_interval_secs.unwrap_or(60));
    
    if continuous {
        info!("Running in continuous mode with {} second intervals", interval);
        run_continuous(&client, &wallet, &config.usdc_mint, database_api.as_ref(), interval).await?;
    } else {
        info!("Running single balance check");
        run_single_check(&client, &wallet, &config.usdc_mint, database_api.as_ref()).await?;
    }
    
    Ok(())
}

async fn run_single_check(
    client: &RpcClient,
    wallet: &Wallet,
    usdc_mint: &str,
    database_api: Option<&DatabaseApi>,
) -> Result<()> {
    let wallet_info = wallet.get_wallet_info(client, usdc_mint)?;
    
    info!("=== Wallet Balance Report ===");
    info!("Wallet: {}", wallet_info.pubkey);
    info!("SOL Balance: {:.6} SOL", wallet_info.sol_balance);
    info!("USDC Balance: {:.2} USDC", wallet_info.usdc_balance);
    info!("Timestamp: {}", wallet_info.timestamp);
    
    // Store in database if available
    if let Some(api) = database_api {
        match api.store_balance(&wallet_info).await {
            Ok(()) => info!("Balance stored in database successfully"),
            Err(e) => error!("Failed to store balance in database: {}", e),
        }
    }
    
    Ok(())
}

async fn run_continuous(
    client: &RpcClient,
    wallet: &Wallet,
    usdc_mint: &str,
    database_api: Option<&DatabaseApi>,
    interval_secs: u64,
) -> Result<()> {
    let mut interval = tokio::time::interval(Duration::from_secs(interval_secs));
    
    loop {
        interval.tick().await;
        
        match run_single_check(client, wallet, usdc_mint, database_api).await {
            Ok(()) => info!("Balance check completed successfully"),
            Err(e) => error!("Balance check failed: {}", e),
        }
        
        info!("Next check in {} seconds...", interval_secs);
    }
} 