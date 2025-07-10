use anyhow::{Result, anyhow};
use reqwest::Client;
use serde_json::json;
use std::env;
use tokio::process::Command;
use tracing::{info, warn, error};
use solana_sdk::signature::Signer;
use dotenv::dotenv;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt::init();
    
    // Load environment variables
    dotenv().ok();
    
    info!("🧪 Starting simple swap test...");
    
    // Get configuration
    let solana_private_key = env::var("SOLANA_PRIVATE_KEY")
        .map_err(|_| anyhow!("SOLANA_PRIVATE_KEY not found in environment"))?;
    
    let database_url = env::var("PRICE_FEED_DATABASE_URL")
        .unwrap_or_else(|_| "http://localhost:8080".to_string());
    
    let transaction_binary_path = "./target/debug/transaction";
    
    info!("📊 Database URL: {}", database_url);
    info!("🔧 Transaction binary: {}", transaction_binary_path);
    
    // Step 1: Get wallet address from private key
    let wallet_address = get_wallet_address(&solana_private_key)?;
    info!("💰 Wallet address: {}", wallet_address);
    
    // Step 2: Execute $1 USDC to SOL swap
    info!("🔄 Executing $1 USDC to SOL swap...");
    let swap_result = execute_swap(transaction_binary_path, 1.0, "usdc-to-sol").await?;
    
    if !swap_result.success {
        error!("❌ Swap failed: {:?}", swap_result.error);
        return Err(anyhow!("Swap failed"));
    }
    
    info!("✅ Swap successful!");
    if let Some(signature) = &swap_result.signature {
        info!("📊 Transaction signature: {}", signature);
    }
    if let (Some(sol_change), Some(usdc_change)) = (swap_result.sol_change, swap_result.usdc_change) {
        info!("💱 Received {:.6} SOL for ${:.2} USDC", sol_change, usdc_change.abs());
    }
    
    // Step 3: Store position to database
    info!("💾 Storing position to database...");
    let position_result = store_position_to_database(&database_url, &wallet_address, 154.64, 1.0).await?;
    
    if position_result {
        info!("✅ Position stored successfully!");
    } else {
        error!("❌ Failed to store position to database");
        return Err(anyhow!("Failed to store position"));
    }
    
    info!("🎉 Test completed successfully!");
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
            if line.contains("Transaction signature:") {
                signature = line.split("Transaction signature:").nth(1)
                    .map(|s| s.trim().to_string());
            } else if line.contains("SOL change:") {
                if let Some(change_str) = line.split("SOL change:").nth(1) {
                    if let Some(number_str) = change_str.split_whitespace().next() {
                        sol_change = number_str.parse::<f64>().ok();
                    }
                }
            } else if line.contains("USDC change:") {
                if let Some(change_str) = line.split("USDC change:").nth(1) {
                    if let Some(number_str) = change_str.split_whitespace().next() {
                        usdc_change = number_str.parse::<f64>().ok();
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

async fn store_position_to_database(database_url: &str, wallet_address: &str, entry_price: f64, quantity: f64) -> Result<bool> {
    let client = Client::new();
    
    let create_position_request = json!({
        "wallet_address": wallet_address,
        "pair": "SOL/USDC",
        "position_type": "long",
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