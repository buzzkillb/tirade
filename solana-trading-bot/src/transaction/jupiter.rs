use solana_sdk::{
    pubkey::Pubkey,
    signature::{Keypair, Signer},
    transaction::{Transaction, VersionedTransaction},
    program_pack::Pack,
};
use solana_sdk::signers::Signers;
use solana_client::rpc_client::RpcClient;
use bincode;
use base64::Engine;
use std::str::FromStr;
use tracing::{info, warn, error};
use crate::transaction::{
    config::Config,
    error::TransactionError,
    types::{Args, JupiterQuote},
};

#[derive(Debug, Clone)]
pub enum TransactionStatus {
    Pending,
    Confirmed,
    Failed,
    NotFound,
    Unknown,
}

// Enhanced balance change detection with tolerance
async fn wait_for_balance_change(
    client: &RpcClient,
    wallet: &Keypair,
    config: &Config,
    initial_sol_balance: f64,
    initial_usdc_balance: f64,
    direction: &str,
    max_attempts: u32,
) -> Result<bool, TransactionError> {
    let mut attempts = 0;
    
    while attempts < max_attempts {
        tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
        attempts += 1;
        
        let current_sol = get_sol_balance(client, &wallet.pubkey())?;
        let current_usdc = get_usdc_balance(client, &wallet.pubkey(), &config.usdc_mint)?;
        
        // Check for expected balance changes with tolerance
        if direction == "usdc-to-sol" {
            if current_sol > (initial_sol_balance + 0.001) { // At least 0.001 SOL received
                info!("✅ Balance change detected! SOL: {:.6} → {:.6} (+{:.6})", 
                      initial_sol_balance, current_sol, current_sol - initial_sol_balance);
                return Ok(true);
            }
        } else {
            if current_usdc > (initial_usdc_balance + 0.01) { // At least 0.01 USDC received
                info!("✅ Balance change detected! USDC: {:.2} → {:.2} (+{:.2})", 
                      initial_usdc_balance, current_usdc, current_usdc - initial_usdc_balance);
                return Ok(true);
            }
        }
        
        info!("⏳ Waiting for balance change (attempt {}/{}): SOL: {:.6}, USDC: {:.2}", 
              attempts, max_attempts, current_sol, current_usdc);
    }
    
    info!("⚠️  No balance change detected after {} attempts", max_attempts);
    Ok(false)
}

// Enhanced transaction status verification
async fn verify_transaction_status(
    client: &RpcClient,
    signature: &str,
) -> Result<TransactionStatus, TransactionError> {
    let signature_sig = solana_sdk::signature::Signature::from_str(signature)
        .map_err(|e| TransactionError::Transaction(format!("Invalid signature: {}", e)))?;
    
    match client.get_signature_status(&signature_sig) {
        Ok(Some(_)) => {
            info!("✅ Transaction confirmed on-chain");
            Ok(TransactionStatus::Confirmed)
        }
        Ok(None) => {
            info!("❌ Transaction not found on-chain");
            Ok(TransactionStatus::NotFound)
        }
        Err(e) => {
            info!("❌ Error checking transaction status: {}", e);
            Ok(TransactionStatus::Unknown)
        }
    }
}

// Enhanced single swap execution with verification
async fn execute_single_swap_with_verification(
    client: &RpcClient,
    wallet: &Keypair,
    quote: &JupiterQuote,
    args: &Args,
    config: &Config,
    initial_sol_balance: f64,
    initial_usdc_balance: f64,
) -> Result<(String, f64, f64), TransactionError> {
    // Execute the swap (existing logic)
    let signature = execute_swap(client, wallet, quote, args, config, initial_sol_balance, initial_usdc_balance).await?;
    
    // Enhanced verification
    let balance_changed = wait_for_balance_change(
        client, wallet, config, initial_sol_balance, initial_usdc_balance, 
        &args.direction, 12
    ).await?;
    
    if !balance_changed {
        // Check transaction status on-chain
        let status = verify_transaction_status(client, &signature).await?;
        match status {
            TransactionStatus::Failed => {
                return Err(TransactionError::Transaction("Transaction failed on-chain".to_string()));
            }
            TransactionStatus::NotFound => {
                return Err(TransactionError::Transaction("Transaction not found on-chain".to_string()));
            }
            _ => {
                return Err(TransactionError::Transaction("Transaction status unclear".to_string()));
            }
        }
    }
    
    // Get final balances to calculate changes
    let final_sol_balance = get_sol_balance(client, &wallet.pubkey())?;
    let final_usdc_balance = get_usdc_balance(client, &wallet.pubkey(), &config.usdc_mint)?;
    
    let sol_change = final_sol_balance - initial_sol_balance;
    let usdc_change = final_usdc_balance - initial_usdc_balance;
    
    Ok((signature, sol_change, usdc_change))
}

// Enhanced swap execution with retry logic
pub async fn execute_swap_with_retry(
    client: &RpcClient,
    wallet: &Keypair,
    quote: &JupiterQuote,
    args: &Args,
    config: &Config,
    initial_sol_balance: f64,
    initial_usdc_balance: f64,
    max_retries: u32,
) -> Result<(String, f64, f64), TransactionError> {
    let mut attempt = 0;
    
    while attempt < max_retries {
        attempt += 1;
        
        info!("🔄 Executing swap attempt {}/{}", attempt, max_retries);
        
        match execute_single_swap_with_verification(
            client, wallet, quote, args, config, 
            initial_sol_balance, initial_usdc_balance
        ).await {
            Ok((signature, sol_change, usdc_change)) => {
                info!("✅ Swap successful on attempt {}", attempt);
                return Ok((signature, sol_change, usdc_change));
            }
            Err(e) => {
                if attempt >= max_retries {
                    error!("❌ All {} retry attempts failed: {}", max_retries, e);
                    return Err(e);
                }
                
                let delay = 2u64.pow(attempt);
                warn!("⚠️  Swap failed (attempt {}/{}), retrying in {} seconds... Error: {}", 
                      attempt, max_retries, delay, e);
                tokio::time::sleep(tokio::time::Duration::from_secs(delay)).await;
            }
        }
    }
    
    Err(TransactionError::Transaction("Max retries exceeded".to_string()))
}

pub async fn get_jupiter_quote(args: &Args, config: &Config) -> Result<JupiterQuote, TransactionError> {
    let client = reqwest::Client::new();
    
    let (input_mint, output_mint, amount) = if args.direction == "usdc-to-sol" {
        (&config.usdc_mint, &config.sol_mint, (args.amount_usdc * 1_000_000.0) as u64)
    } else {
        (&config.sol_mint, &config.usdc_mint, (args.amount_usdc * 1_000_000_000.0) as u64)
    };
    
    let url = format!(
        "{}/quote?inputMint={}&outputMint={}&amount={}&slippageBps={}",
        config.jupiter_base_url, input_mint, output_mint, amount, args.slippage_bps
    );
    
    info!("Requesting quote from Jupiter: {}", url);
    
    let response = client.get(&url).send().await
        .map_err(|e| TransactionError::JupiterApi(format!("Request failed: {}", e)))?;
    
    if !response.status().is_success() {
        return Err(TransactionError::JupiterApi(format!("API error: {}", response.status())));
    }
    
    let quote_data: serde_json::Value = response.json().await
        .map_err(|e| TransactionError::JupiterApi(format!("JSON parse error: {}", e)))?;
    
    let input_amount = quote_data["inAmount"]
        .as_str()
        .unwrap_or("0")
        .to_string();
    let output_amount = quote_data["outAmount"]
        .as_str()
        .unwrap_or("0")
        .to_string();
    let price_impact = quote_data["priceImpactPct"]
        .as_f64()
        .unwrap_or(0.0);
    let routes = quote_data["routes"]
        .as_array()
        .unwrap_or(&Vec::new())
        .clone();
    
    // Calculate actual execution price from Jupiter amounts
    let execution_price = if args.direction == "usdc-to-sol" {
        // For BUY: Price = USDC spent / SOL received
        let usdc_amount = input_amount.parse::<f64>().unwrap_or(0.0) / 1_000_000.0;
        let sol_amount = output_amount.parse::<f64>().unwrap_or(0.0) / 1_000_000_000.0;
        if sol_amount > 0.0 { 
            usdc_amount / sol_amount 
        } else { 
            0.0 
        }
    } else {
        // For SELL: Price = USDC received / SOL spent
        let sol_amount = input_amount.parse::<f64>().unwrap_or(0.0) / 1_000_000_000.0;
        let usdc_amount = output_amount.parse::<f64>().unwrap_or(0.0) / 1_000_000.0;
        if sol_amount > 0.0 { 
            usdc_amount / sol_amount 
        } else { 
            0.0 
        }
    };
    
    info!("📊 Jupiter execution price: ${:.4} per SOL (direction: {})", execution_price, args.direction);
    info!("📊 Input: {} | Output: {} | Price Impact: {:.2}%", 
          if args.direction == "usdc-to-sol" { 
              format!("${:.2} USDC", input_amount.parse::<f64>().unwrap_or(0.0) / 1_000_000.0)
          } else { 
              format!("{:.6} SOL", input_amount.parse::<f64>().unwrap_or(0.0) / 1_000_000_000.0)
          },
          if args.direction == "usdc-to-sol" { 
              format!("{:.6} SOL", output_amount.parse::<f64>().unwrap_or(0.0) / 1_000_000_000.0)
          } else { 
              format!("${:.2} USDC", output_amount.parse::<f64>().unwrap_or(0.0) / 1_000_000.0)
          },
          price_impact * 100.0);
    
    Ok(JupiterQuote {
        input_amount,
        output_amount,
        execution_price,
        price_impact,
        routes,
        quote_data,
    })
}

pub async fn execute_swap(
    client: &RpcClient,
    wallet: &Keypair,
    quote: &JupiterQuote,
    args: &Args,
    config: &Config,
    initial_sol_balance: f64,
    initial_usdc_balance: f64,
) -> Result<String, TransactionError> {
    let http_client = reqwest::Client::new();
    
    // Step 1: Get swap transaction from Jupiter
    let swap_url = format!("{}/swap", config.jupiter_base_url);
    
    let swap_request = serde_json::json!({
        "quoteResponse": quote.quote_data,
        "userPublicKey": wallet.pubkey().to_string(),
        "wrapUnwrapSOL": true
    });
    
    info!("Requesting swap transaction from Jupiter...");
    
    let swap_response = http_client
        .post(&swap_url)
        .json(&swap_request)
        .send()
        .await
        .map_err(|e| TransactionError::JupiterApi(format!("Swap request failed: {}", e)))?;
    
    let status = swap_response.status();
    if !status.is_success() {
        let error_text = swap_response.text().await
            .unwrap_or_else(|_| "Unknown error".to_string());
        return Err(TransactionError::JupiterApi(format!("Swap API error: {} - {}", status, error_text)));
    }
    
    let swap_data: serde_json::Value = swap_response.json().await
        .map_err(|e| TransactionError::JupiterApi(format!("Swap response parse error: {}", e)))?;
    
    // Extract the serialized transaction
    let serialized_transaction = swap_data["swapTransaction"]
        .as_str()
        .ok_or_else(|| TransactionError::JupiterApi("No swapTransaction in response".to_string()))?;
    
    info!("Received serialized transaction (first 100 chars): {}", 
          &serialized_transaction[..std::cmp::min(100, serialized_transaction.len())]);
    
    // Step 2: Deserialize and sign the transaction (using versioned format)
    let transaction_bytes = base64::engine::general_purpose::STANDARD.decode(serialized_transaction)?;
    
    // Deserialize as VersionedTransaction
    let mut transaction: VersionedTransaction = bincode::deserialize(&transaction_bytes)?;
    
    // Sign the transaction properly
    // The transaction from Jupiter already has the correct blockhash and structure
    // We just need to sign it with our wallet
    let message_data = transaction.message.serialize();
    let signature = wallet.sign_message(&message_data);
    
    // Replace the first signature (which should be our wallet's signature)
    if !transaction.signatures.is_empty() {
        transaction.signatures[0] = signature;
    } else {
        transaction.signatures.push(signature);
    }
    
    // Step 3: Send the transaction
    info!("Sending transaction to Solana network...");
    
    // For VersionedTransaction, we need to use send_transaction and then confirm separately
    let signature = client.send_transaction(&transaction)
        .map_err(|e| TransactionError::Transaction(format!("Transaction failed: {}", e)))?;
    
    // Wait for confirmation
    client.confirm_transaction(&signature)
        .map_err(|e| TransactionError::Transaction(format!("Transaction confirmation failed: {}", e)))?;
    
    info!("Transaction sent successfully!");
    info!("Signature: {}", signature);
    
    // Wait for transaction confirmation and then poll for balance changes
    info!("Waiting for transaction confirmation...");
    client.confirm_transaction(&signature)
        .map_err(|e| TransactionError::Transaction(format!("Transaction confirmation failed: {}", e)))?;
    
    info!("Transaction confirmed! Polling for balance changes...");
    
    // Poll for balance changes every 5 seconds for up to 60 seconds
    let mut attempts = 0;
    let max_attempts = 12; // 60 seconds / 5 seconds
    
    while attempts < max_attempts {
        tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
        attempts += 1;
        
        info!("Checking balance update (attempt {}/{}):", attempts, max_attempts);
        
        // Check if balances have changed
        let current_sol = get_sol_balance(client, &wallet.pubkey())?;
        let current_usdc = get_usdc_balance(client, &wallet.pubkey(), &config.usdc_mint)?;
        
        info!("  Current SOL: {:.6} SOL", current_sol);
        info!("  Current USDC: {:.2} USDC", current_usdc);
        
        // If this is a USDC to SOL swap, check if SOL increased
        if args.direction == "usdc-to-sol" {
            if current_sol > initial_sol_balance {
                info!("✅ Balance change detected! SOL increased from {:.6} to {:.6}", initial_sol_balance, current_sol);
                break;
            }
        } else {
            // SOL to USDC swap, check if USDC increased
            if current_usdc > initial_usdc_balance {
                info!("✅ Balance change detected! USDC increased from {:.2} to {:.2}", initial_usdc_balance, current_usdc);
                break;
            }
        }
        
        if attempts >= max_attempts {
            info!("⚠️  Balance change not detected after {} attempts. Transaction may still be processing.", max_attempts);
        }
    }
    
    Ok(signature.to_string())
}

pub fn get_sol_balance(client: &RpcClient, pubkey: &Pubkey) -> Result<f64, TransactionError> {
    let balance = client.get_balance(pubkey)
        .map_err(|e| TransactionError::SolanaRpc(format!("Failed to get SOL balance: {}", e)))?;
    Ok(balance as f64 / 1_000_000_000.0)
}

pub fn get_usdc_balance(client: &RpcClient, pubkey: &Pubkey, usdc_mint: &str) -> Result<f64, TransactionError> {
    let mint_pubkey = Pubkey::from_str(usdc_mint)?;
    let accounts = client.get_token_accounts_by_owner(
        pubkey, 
        solana_client::rpc_request::TokenAccountsFilter::Mint(mint_pubkey)
    ).map_err(|e| TransactionError::SolanaRpc(format!("Failed to get USDC accounts: {}", e)))?;
    
    let mut total_balance = 0.0;
    for account in accounts {
        match &account.account.data {
            solana_account_decoder::UiAccountData::Binary(data, encoding) if *encoding == solana_account_decoder::UiAccountEncoding::Base64 => {
                if let Ok(decoded) = base64::engine::general_purpose::STANDARD.decode(data) {
                    if let Ok(token_account) = spl_token::state::Account::unpack(&decoded) {
                        total_balance += token_account.amount as f64 / 1_000_000.0;
                    }
                }
            }
            solana_account_decoder::UiAccountData::Json(data) => {
                if data.program == "spl-token" {
                    if let Some(info) = data.parsed.get("info") {
                        if let Some(token_amount) = info.get("tokenAmount") {
                            if let Some(amount_str) = token_amount.get("amount") {
                                if let Ok(amount) = amount_str.as_str().unwrap_or("0").parse::<f64>() {
                                    total_balance += amount / 1_000_000.0;
                                }
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }
    Ok(total_balance)
} 