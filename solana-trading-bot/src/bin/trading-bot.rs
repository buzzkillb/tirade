use clap::Parser;
use tracing::info;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Trading strategy to use
    #[arg(short, long, default_value = "simple")]
    strategy: String,
    
    /// Run in backtest mode
    #[arg(short, long)]
    backtest: bool,
    
    /// Initial capital in SOL
    #[arg(short, long, default_value = "1.0")]
    capital: f64,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt::init();
    
    // Load environment variables
    dotenv::dotenv().ok();
    
    // Parse command line arguments
    let args = Args::parse();
    
    info!("Starting Solana Trading Bot");
    info!("Strategy: {}", args.strategy);
    info!("Mode: {}", if args.backtest { "Backtest" } else { "Live" });
    info!("Initial Capital: {} SOL", args.capital);
    
    // TODO: Initialize trading components
    // - Connect to Solana RPC
    // - Load wallet/private key
    // - Initialize trading strategy
    // - Set up order management
    // - Start market data feeds
    
    info!("Trading bot initialized successfully");
    info!("Ready to start trading...");
    
    // TODO: Main trading loop
    // - Monitor market conditions
    // - Execute trading signals
    // - Manage positions
    // - Risk management
    
    Ok(())
} 