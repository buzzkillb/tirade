use solana_client::rpc_client::RpcClient;
use solana_sdk::{
    pubkey::Pubkey,
    signature::{Keypair, Signer},
    transaction::Transaction,
    program_pack::Pack,
};
use base64::Engine;
use std::str::FromStr;
use tracing::info;

use crate::transaction::{
    config::Config,
    error::TransactionError,
    types::{Args, JupiterQuote},
};

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
    
    Ok(JupiterQuote {
        input_amount,
        output_amount,
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
) -> Result<String, TransactionError> {
    let http_client = reqwest::Client::new();
    
    // Step 1: Get swap transaction from Jupiter
    let swap_url = format!("{}/swap", config.jupiter_base_url);
    
    let swap_request = serde_json::json!({
        "quoteResponse": quote.quote_data,
        "userPublicKey": wallet.pubkey().to_string(),
        "wrapUnwrapSOL": true,
        "asLegacyTransaction": true
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
    
    // Step 2: Deserialize and sign the transaction (using legacy format)
    let transaction_bytes = base64::engine::general_purpose::STANDARD.decode(serialized_transaction)?;
    
    // Deserialize as regular Transaction (legacy format)
    let mut transaction: Transaction = bincode::deserialize(&transaction_bytes)?;
    
    // Get recent blockhash and update transaction
    let recent_blockhash = client.get_latest_blockhash()
        .map_err(|e| TransactionError::SolanaRpc(format!("Failed to get blockhash: {}", e)))?;
    transaction.message.recent_blockhash = recent_blockhash;
    
    // Sign the transaction
    transaction.sign(&[wallet], recent_blockhash);
    
    // Step 3: Send the transaction
    info!("Sending transaction to Solana network...");
    
    let signature = client.send_and_confirm_transaction(&transaction)
        .map_err(|e| TransactionError::Transaction(format!("Transaction failed: {}", e)))?;
    
    info!("Transaction sent successfully!");
    info!("Signature: {}", signature);
    
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