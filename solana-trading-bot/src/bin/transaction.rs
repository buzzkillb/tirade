use solana_trading_bot::transaction::{
    Args, Config, TransactionError,
    config::create_wallet_from_private_key,
    jupiter::{execute_swap, execute_swap_with_retry, get_jupiter_quote, get_sol_balance, get_usdc_balance},
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
    
    // Check if we have enough SOL for transaction fees (at least 0.01 SOL)
    if sol_balance < 0.01 {
        return Err(TransactionError::Balance(format!("Insufficient SOL for transaction fees. Have: {:.6} SOL, Need at least 0.01 SOL", sol_balance)));
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
    
    // Execute the swap with enhanced retry logic
    info!("Executing swap transaction with retry logic...");
    let (tx_signature, sol_change, usdc_change) = execute_swap_with_retry(
        &client, &wallet, &quote, &args, &config, 
        sol_balance, usdc_balance, 3 // 3 retries
    ).await?;
    info!("Transaction successful! Signature: {}", tx_signature);
    
    // Calculate new balances
    let new_sol_balance = sol_balance + sol_change;
    let new_usdc_balance = usdc_balance + usdc_change;
    
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
    if sol_change > 0.0 {
        info!("  SOL: {:.6} SOL (received)", sol_change);
    } else {
        info!("  SOL: {:.6} SOL (spent)", sol_change.abs());
    }
    if usdc_change > 0.0 {
        info!("  USDC: {:.2} USDC (received)", usdc_change);
    } else {
        info!("  USDC: {:.2} USDC (spent)", usdc_change.abs());
    }
    info!("");
    info!("EXECUTION PRICE:");
    info!("  Jupiter Execution Price: ${:.4} per SOL", quote.execution_price);
    info!("");
    info!("JUPITER QUOTE vs ACTUAL:");
    let quoted_sol = quote.output_amount.parse::<f64>().unwrap_or(0.0) / 1_000_000_000.0;
    info!("  Quoted SOL: {:.6} SOL", quoted_sol);
    info!("  Actual SOL: {:.6} SOL", sol_change.abs());
    info!("  Difference: {:.6} SOL ({:.4}%)", 
          sol_change.abs() - quoted_sol, 
          ((sol_change.abs() - quoted_sol) / quoted_sol * 100.0));
    
    Ok(())
}

 