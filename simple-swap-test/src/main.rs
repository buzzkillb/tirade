use anyhow::{Result, anyhow};
use reqwest::Client;
use serde_json::json;
use std::env;
use tokio::process::Command;
use tracing::{info, warn, error};
use solana_sdk::signature::Signer;
use dotenv::dotenv;

#[derive(Debug)]
struct WalletBalance {
    sol_balance: f64,
    usdc_balance: f64,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt::init();
    
    // Load environment variables
    dotenv().ok();
    
    info!("🧪 Starting enhanced simple swap test...");
    
    // Get configuration
    let solana_private_key = env::var("SOLANA_PRIVATE_KEY")
        .map_err(|_| anyhow!("SOLANA_PRIVATE_KEY not found in environment"))?;
    
    let database_url = env::var("DATABASE_URL")
        .unwrap_or_else(|_| "http://localhost:8080".to_string());
    
    // If DATABASE_URL is a file path (SQLite), use the HTTP endpoint instead
    let database_url = if database_url.starts_with("sqlite:") || database_url.ends_with(".db") {
        "http://localhost:8080".to_string()
    } else {
        database_url
    };
    
    let transaction_binary_path = "../target/debug/transaction";
    
    info!("📊 Database URL: {}", database_url);
    info!("🔧 Transaction binary: {}", transaction_binary_path);
    
    // Step 1: Get wallet address from private key
    let wallet_address = get_wallet_address(&solana_private_key)?;
    info!("💰 Wallet address: {}", wallet_address);
    
    // Step 2: Create wallet in database first
    info!("💾 Creating wallet in database...");
    let wallet_created = create_wallet_in_database(&database_url, &wallet_address).await?;
    if !wallet_created {
        warn!("⚠️  Wallet creation failed, but continuing...");
    }
    
    // Step 3: Get initial balances
    info!("📊 Getting initial balances...");
    let initial_balance = get_wallet_balance(&transaction_binary_path).await?;
    info!("💰 Initial balances:");
    info!("   SOL: {:.6} SOL", initial_balance.sol_balance);
    info!("   USDC: {:.2} USDC", initial_balance.usdc_balance);
    
    // Step 4: Execute USDC to SOL swap
    let test_amount_usdc = 1.0;
    info!("🔄 Step 1: Executing ${:.2} USDC to SOL swap...", test_amount_usdc);
    let usdc_to_sol_result = execute_swap(transaction_binary_path, test_amount_usdc, "usdc-to-sol").await?;
    
    if !usdc_to_sol_result.success {
        error!("❌ USDC to SOL swap failed: {:?}", usdc_to_sol_result.error);
        return Err(anyhow!("USDC to SOL swap failed"));
    }
    
    let sol_received = usdc_to_sol_result.sol_change.unwrap_or(0.0);
    info!("✅ USDC to SOL swap successful!");
    info!("   Received: {:.6} SOL", sol_received);
    if let Some(signature) = &usdc_to_sol_result.signature {
        info!("   Transaction signature: {}", signature);
    }
    
    // Step 5: Store buy position to database
    info!("💾 Storing buy position to database...");
    let buy_position_result = store_position_to_database(&database_url, &wallet_address, 154.64, sol_received, "long").await?;
    if !buy_position_result {
        error!("❌ Failed to store buy position to database");
        return Err(anyhow!("Failed to store buy position"));
    }
    
    // Step 6: Execute SOL to USDC swap
    info!("🔄 Step 2: Executing {:.6} SOL to USDC swap...", sol_received);
    let sol_to_usdc_result = execute_swap(transaction_binary_path, sol_received, "sol-to-usdc").await?;
    
    if !sol_to_usdc_result.success {
        error!("❌ SOL to USDC swap failed: {:?}", sol_to_usdc_result.error);
        return Err(anyhow!("SOL to USDC swap failed"));
    }
    
    let usdc_received = sol_to_usdc_result.usdc_change.unwrap_or(0.0);
    info!("✅ SOL to USDC swap successful!");
    info!("   Received: {:.2} USDC", usdc_received);
    if let Some(signature) = &sol_to_usdc_result.signature {
        info!("   Transaction signature: {}", signature);
    }
    
    // Step 7: Calculate and display PnL
    info!("💰 PnL Analysis:");
    let pnl = usdc_received - test_amount_usdc;
    let pnl_percent = (pnl / test_amount_usdc) * 100.0;
    
    info!("   USDC Spent: ${:.2}", test_amount_usdc);
    info!("   USDC Received: ${:.2}", usdc_received);
    info!("   Net PnL: ${:.2} USDC", pnl);
    info!("   PnL %: {:.2}%", pnl_percent);
    
    if pnl > 0.0 {
        info!("✅ PROFIT: +${:.2} USDC ({:.2}%)", pnl, pnl_percent);
    } else if pnl < 0.0 {
        info!("💸 LOSS: ${:.2} USDC ({:.2}%)", pnl, pnl_percent);
    } else {
        info!("➡️  BREAKEVEN: ${:.2} USDC", pnl);
    }
    
    // Step 8: Store sell position to database
    info!("💾 Storing sell position to database...");
    let sell_position_result = store_position_to_database(&database_url, &wallet_address, 154.64, usdc_received, "short").await?;
    if !sell_position_result {
        error!("❌ Failed to store sell position to database");
        return Err(anyhow!("Failed to store sell position"));
    }
    
    // Step 9: Get final balances
    info!("📊 Getting final balances...");
    let final_balance = get_wallet_balance(&transaction_binary_path).await?;
    info!("💰 Final balances:");
    info!("   SOL: {:.6} SOL", final_balance.sol_balance);
    info!("   USDC: {:.2} USDC", final_balance.usdc_balance);
    
    // Step 10: Balance consistency check
    info!("🔍 Balance Consistency Check:");
    let expected_sol_change = sol_received;
    let expected_usdc_change = usdc_received - test_amount_usdc;
    
    let actual_sol_change = final_balance.sol_balance - initial_balance.sol_balance;
    let actual_usdc_change = final_balance.usdc_balance - initial_balance.usdc_balance;
    
    info!("   Expected SOL change: {:.6} SOL", expected_sol_change);
    info!("   Actual SOL change: {:.6} SOL", actual_sol_change);
    info!("   Expected USDC change: {:.2} USDC", expected_usdc_change);
    info!("   Actual USDC change: {:.2} USDC", actual_usdc_change);
    
    // Check if changes are reasonable (accounting for fees and slippage)
    let sol_tolerance = 0.0001; // 0.0001 SOL tolerance
    let usdc_tolerance = 0.01; // $0.01 USDC tolerance
    
    let sol_diff = (actual_sol_change - expected_sol_change).abs();
    let usdc_diff = (actual_usdc_change - expected_usdc_change).abs();
    
    if sol_diff <= sol_tolerance && usdc_diff <= usdc_tolerance {
        info!("✅ Balance consistency verified");
    } else {
        info!("⚠️  Balance inconsistency detected:");
        info!("   SOL difference: {:.6} SOL", sol_diff);
        info!("   USDC difference: {:.2} USDC", usdc_diff);
    }
    
    info!("🎉 Enhanced simple swap test completed successfully!");
    Ok(())
}

fn get_wallet_address(private_key: &str) -> Result<String> {
    let private_key = private_key.trim();
    let keypair_bytes = if private_key.starts_with('[') {
        serde_json::from_str::<Vec<u8>>(private_key)
            .map_err(|e| anyhow!("Invalid SOLANA_PRIVATE_KEY format: {}", e))?
    } else {
        bs58::decode(private_key)
            .into_vec()
            .map_err(|e| anyhow!("Invalid base58 private key: {}", e))?
    };
    
    if keypair_bytes.len() != 64 {
        return Err(anyhow!("Invalid private key length"));
    }
    
    let keypair = solana_sdk::signature::Keypair::from_bytes(&keypair_bytes)
        .map_err(|e| anyhow!("Failed to create keypair: {}", e))?;
    
    Ok(keypair.pubkey().to_string())
}

async fn execute_swap(binary_path: &str, amount: f64, direction: &str) -> Result<SwapResult> {
    let solana_rpc_url = env::var("SOLANA_RPC_URL")
        .unwrap_or_else(|_| "https://api.mainnet-beta.solana.com".to_string());
    
    let solana_private_key = env::var("SOLANA_PRIVATE_KEY")
        .map_err(|_| anyhow!("SOLANA_PRIVATE_KEY not found in environment"))?;
    
    let slippage_tolerance = env::var("SLIPPAGE_TOLERANCE")
        .unwrap_or_else(|_| "0.005".to_string())
        .parse::<f64>()
        .unwrap_or(0.005);
    
    let slippage_bps = (slippage_tolerance * 10000.0) as u32;
    
    let mut cmd = Command::new(binary_path);
    cmd.env("SOLANA_RPC_URL", &solana_rpc_url)
       .env("SOLANA_PRIVATE_KEY", &solana_private_key)
       .arg("--amount-usdc")
       .arg(&amount.to_string())
       .arg("--direction")
       .arg(direction)
       .arg("--slippage-bps")
       .arg(&slippage_bps.to_string());

    info!("🔧 Executing command: {} {:?}", binary_path, cmd.as_std().get_args().collect::<Vec<_>>());

    let output = cmd.output().await?;

    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        info!("📄 Command output:\n{}", stdout);
        
        // Parse the output to extract transaction details
        let mut signature = None;
        let mut sol_change = None;
        let mut usdc_change = None;
        
        for line in stdout.lines() {
            if line.contains("Transaction Signature:") || line.contains("Signature:") {
                signature = line.split(':').nth(1).map(|s| s.trim().to_string());
            } else if line.contains("SOL: ") && line.contains("(received)") {
                if let Some(change_str) = line.split("SOL:").nth(1) {
                    if let Some(num_str) = change_str.split("SOL").next() {
                        sol_change = num_str.trim().parse::<f64>().ok();
                    }
                }
            } else if line.contains("USDC: ") && (line.contains("(spent)") || line.contains("(received)")) {
                if let Some(change_str) = line.split("USDC:").nth(1) {
                    if let Some(num_str) = change_str.split("USDC").next() {
                        let mut change = num_str.trim().parse::<f64>().unwrap_or(0.0);
                        if line.contains("(spent)") {
                            change = -change; // Make spent amounts negative
                        }
                        usdc_change = Some(change);
                    }
                }
            }
        }
        
        Ok(SwapResult {
            success: true,
            signature,
            error: None,
            sol_change,
            usdc_change,
        })
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        error!("❌ Command failed with status: {}", output.status);
        error!("📄 stdout: {}", stdout);
        error!("📄 stderr: {}", stderr);
        
        Ok(SwapResult {
            success: false,
            signature: None,
            error: Some(format!("Command failed: {}", stderr)),
            sol_change: None,
            usdc_change: None,
        })
    }
}

async fn create_wallet_in_database(database_url: &str, wallet_address: &str) -> Result<bool> {
    let client = Client::new();
    
    let create_wallet_request = json!({
        "address": wallet_address,
    });

    info!("📤 Creating wallet in database:");
    info!("   URL: {}/wallets", database_url);
    info!("   Request: {}", serde_json::to_string_pretty(&create_wallet_request)?);

    let response = client.post(&format!("{}/wallets", database_url))
        .json(&create_wallet_request)
        .send()
        .await?;

    info!("📥 Database response status: {}", response.status());

    if response.status().is_success() {
        let response_text = response.text().await?;
        info!("📄 Database response: {}", response_text);
        Ok(true)
    } else {
        let error_text = response.text().await?;
        warn!("⚠️  Wallet creation error (this might be expected if wallet already exists): {}", error_text);
        // Don't fail if wallet already exists
        Ok(true)
    }
}

async fn get_wallet_balance(transaction_binary_path: &str) -> Result<WalletBalance> {
    let solana_rpc_url = env::var("SOLANA_RPC_URL")
        .unwrap_or_else(|_| "https://api.mainnet-beta.solana.com".to_string());
    
    let solana_private_key = env::var("SOLANA_PRIVATE_KEY")
        .map_err(|_| anyhow!("SOLANA_PRIVATE_KEY not found in environment"))?;
    
    let mut cmd = Command::new(transaction_binary_path);
    cmd.env("SOLANA_RPC_URL", &solana_rpc_url)
       .env("SOLANA_PRIVATE_KEY", &solana_private_key)
       .arg("--amount-usdc")
       .arg("0.01") // Small amount just to check balances
       .arg("--direction")
       .arg("usdc-to-sol")
       .arg("--dry-run"); // Always dry-run for balance checking

    let output = cmd.output().await?;

    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        
        // Parse the balance information from the output
        let mut sol_balance = 0.0;
        let mut usdc_balance = 0.0;
        
        for line in stdout.lines() {
            if line.contains("SOL:") && line.contains("SOL") {
                // Look for lines like "  SOL: 1.234567 SOL"
                if let Some(balance_part) = line.split("SOL:").nth(1) {
                    if let Some(number_part) = balance_part.split("SOL").next() {
                        sol_balance = number_part.trim().parse::<f64>().unwrap_or(0.0);
                    }
                }
            } else if line.contains("USDC:") && line.contains("USDC") {
                // Look for lines like "  USDC: 100.00 USDC"  
                if let Some(balance_part) = line.split("USDC:").nth(1) {
                    if let Some(number_part) = balance_part.split("USDC").next() {
                        usdc_balance = number_part.trim().parse::<f64>().unwrap_or(0.0);
                    }
                }
            }
        }

        Ok(WalletBalance {
            sol_balance,
            usdc_balance,
        })
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(anyhow!("Failed to get wallet balance: {}", stderr))
    }
}

async fn store_position_to_database(database_url: &str, wallet_address: &str, entry_price: f64, quantity: f64, position_type: &str) -> Result<bool> {
    let client = Client::new();
    
    let create_position_request = json!({
        "wallet_address": wallet_address,
        "pair": "SOL/USDC",
        "position_type": position_type,
        "entry_price": entry_price,
        "quantity": quantity,
    });

    info!("📤 Sending position request to database:");
    info!("   URL: {}/positions", database_url);
    info!("   Request: {}", serde_json::to_string_pretty(&create_position_request)?);

    let response = client.post(&format!("{}/positions", database_url))
        .json(&create_position_request)
        .send()
        .await?;

    info!("📥 Database response status: {}", response.status());

    if response.status().is_success() {
        let response_text = response.text().await?;
        info!("📄 Database response: {}", response_text);
        Ok(true)
    } else {
        let error_text = response.text().await?;
        error!("❌ Database error: {}", error_text);
        Ok(false)
    }
}

#[derive(Debug)]
struct SwapResult {
    success: bool,
    signature: Option<String>,
    error: Option<String>,
    sol_change: Option<f64>,
    usdc_change: Option<f64>,
}