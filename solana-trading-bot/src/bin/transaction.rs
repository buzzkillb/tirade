use solana_trading_bot::transaction::{
    Args, Config, TransactionError,
    config::create_wallet_from_private_key,
    jupiter::{execute_swap, get_jupiter_quote, get_sol_balance, get_usdc_balance},
};

use clap::Parser;
use solana_client::rpc_client::RpcClient;
use solana_sdk::signer::Signer;
use tracing::info;


#[tokio::main]
async fn main() -> Result<(), TransactionError> {
    // Initialize logging
    tracing_subscriber::fmt::init();
    
    // Load environment variables
    dotenv::dotenv().ok();
    
    // Parse command line arguments
    let args = Args::parse();
    
    info!("Starting Solana Transaction Test");
    info!("Direction: {}", args.direction);
    info!("Amount: {} USDC", args.amount_usdc);
    info!("Slippage: {} bps ({}%)", args.slippage_bps, args.slippage_bps as f64 / 100.0);
    info!("Dry run: {}", args.dry_run);
    
    // Load configuration
    let config = Config::load()?;
    
    // Create Solana client
    let client = RpcClient::new(config.rpc_url.clone());
    
    // Create wallet from private key
    let wallet = create_wallet_from_private_key(&config.private_key)?;
    info!("Wallet: {}", wallet.pubkey());
    
    // Check current balances
    let sol_balance = get_sol_balance(&client, &wallet.pubkey())?;
    let usdc_balance = get_usdc_balance(&client, &wallet.pubkey(), &config.usdc_mint)?;
    
    info!("Current balances:");
    info!("  SOL: {:.6} SOL", sol_balance);
    info!("  USDC: {:.2} USDC", usdc_balance);
    
    // Validate we have enough balance
    if args.direction == "usdc-to-sol" && usdc_balance < args.amount_usdc {
        return Err(TransactionError::Balance(format!("Insufficient USDC balance. Have: {:.2}, Need: {:.2}", 
                          usdc_balance, args.amount_usdc)));
    }
    
    // Get quote from Jupiter
    let quote = get_jupiter_quote(&args, &config).await?;
    info!("Jupiter quote received:");
    info!("  Input amount: {} {}", quote.input_amount, if args.direction == "usdc-to-sol" { "USDC" } else { "SOL" });
    info!("  Output amount: {} {}", quote.output_amount, if args.direction == "usdc-to-sol" { "SOL" } else { "USDC" });
    info!("  Price impact: {:.4}%", quote.price_impact);
    
    if args.dry_run {
        info!("DRY RUN - No transaction will be executed");
        return Ok(());
    }
    
    // Execute the swap
    info!("Executing swap transaction...");
    let tx_signature = execute_swap(&client, &wallet, &quote, &args, &config, sol_balance, usdc_balance).await?;
    info!("Transaction successful! Signature: {}", tx_signature);
    
    // Wait for transaction confirmation and check new balances
    info!("Waiting for transaction confirmation...");
    tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;
    
    let new_sol_balance = get_sol_balance(&client, &wallet.pubkey())?;
    let new_usdc_balance = get_usdc_balance(&client, &wallet.pubkey(), &config.usdc_mint)?;
    
    let sol_change = new_sol_balance - sol_balance;
    let usdc_change = new_usdc_balance - usdc_balance;
    
    info!("=== TRANSACTION RESULTS ===");
    info!("Transaction Signature: {}", tx_signature);
    info!("");
    info!("BEFORE:");
    info!("  SOL: {:.6} SOL", sol_balance);
    info!("  USDC: {:.2} USDC", usdc_balance);
    info!("");
    info!("AFTER:");
    info!("  SOL: {:.6} SOL", new_sol_balance);
    info!("  USDC: {:.2} USDC", new_usdc_balance);
    info!("");
    info!("CHANGES:");
    info!("  SOL: {:.6} SOL (received)", sol_change);
    info!("  USDC: {:.2} USDC (spent)", usdc_change.abs());
    info!("");
    info!("JUPITER QUOTE vs ACTUAL:");
    let quoted_sol = quote.output_amount.parse::<f64>().unwrap_or(0.0) / 1_000_000_000.0;
    info!("  Quoted SOL: {:.6} SOL", quoted_sol);
    info!("  Actual SOL: {:.6} SOL", sol_change);
    info!("  Difference: {:.6} SOL ({:.4}%)", 
          sol_change - quoted_sol, 
          ((sol_change - quoted_sol) / quoted_sol * 100.0));
    
    Ok(())
}

 