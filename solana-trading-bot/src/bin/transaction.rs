use clap::Parser;
use solana_client::rpc_client::RpcClient;
use solana_sdk::{
    pubkey::Pubkey,
    signature::{Keypair, Signer},
    program_pack::Pack,
    transaction::Transaction,
};
use std::str::FromStr;
use tracing::info;
use base64::Engine;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Amount in USDC to swap (default: 0.10)
    #[arg(short, long, default_value = "0.10")]
    amount_usdc: f64,
    
    /// Direction: usdc-to-sol or sol-to-usdc
    #[arg(long, default_value = "usdc-to-sol")]
    direction: String,
    
    /// Dry run - don't actually execute the transaction
    #[arg(short, long)]
    dry_run: bool,
    
    /// Slippage tolerance in basis points (default: 50 = 0.5%)
    #[arg(short, long, default_value = "50")]
    slippage_bps: u32,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
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
    let config = load_config()?;
    
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
        return Err(format!("Insufficient USDC balance. Have: {:.2}, Need: {:.2}", 
                          usdc_balance, args.amount_usdc).into());
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
    let tx_signature = execute_swap(&client, &wallet, &quote, &args, &config).await?;
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

#[derive(Debug)]
struct Config {
    rpc_url: String,
    private_key: String,
    usdc_mint: String,
    sol_mint: String,
    jupiter_base_url: String,
}

fn load_config() -> Result<Config, Box<dyn std::error::Error>> {
    let rpc_url = std::env::var("SOLANA_RPC_URL")
        .unwrap_or_else(|_| "https://api.mainnet-beta.solana.com".to_string());
    
    let private_key = std::env::var("SOLANA_PRIVATE_KEY")
        .map_err(|_| "SOLANA_PRIVATE_KEY not found in .env file")?;
    
    let usdc_mint = std::env::var("USDC_MINT")
        .unwrap_or_else(|_| "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v".to_string());
    
    let sol_mint = std::env::var("SOL_MINT")
        .unwrap_or_else(|_| "So11111111111111111111111111111111111111112".to_string());
    
    Ok(Config {
        rpc_url,
        private_key,
        usdc_mint,
        sol_mint,
        jupiter_base_url: "https://quote-api.jup.ag/v6".to_string(),
    })
}

fn create_wallet_from_private_key(private_key: &str) -> Result<Keypair, Box<dyn std::error::Error>> {
    let private_key = private_key.trim();
    let keypair_bytes = if private_key.starts_with('[') {
        serde_json::from_str::<Vec<u8>>(private_key)?
    } else {
        bs58::decode(private_key).into_vec()?
    };
    
    if keypair_bytes.len() != 64 {
        return Err("Invalid private key length".into());
    }
    
    let keypair = Keypair::from_bytes(&keypair_bytes)?;
    Ok(keypair)
}

fn get_sol_balance(client: &RpcClient, pubkey: &Pubkey) -> Result<f64, Box<dyn std::error::Error>> {
    let balance = client.get_balance(pubkey)?;
    Ok(balance as f64 / 1_000_000_000.0)
}

fn get_usdc_balance(client: &RpcClient, pubkey: &Pubkey, usdc_mint: &str) -> Result<f64, Box<dyn std::error::Error>> {
    let mint_pubkey = Pubkey::from_str(usdc_mint)?;
    let accounts = client.get_token_accounts_by_owner(
        pubkey, 
        solana_client::rpc_request::TokenAccountsFilter::Mint(mint_pubkey)
    )?;
    
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

#[derive(Debug)]
struct JupiterQuote {
    input_amount: String,
    output_amount: String,
    price_impact: f64,
    #[allow(dead_code)]
    routes: Vec<serde_json::Value>,
    quote_data: serde_json::Value, // Store the full quote response
}

async fn get_jupiter_quote(args: &Args, config: &Config) -> Result<JupiterQuote, Box<dyn std::error::Error>> {
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
    
    let response = client.get(&url).send().await?;
    if !response.status().is_success() {
        return Err(format!("Jupiter API error: {}", response.status()).into());
    }
    
    let quote_data: serde_json::Value = response.json().await?;
    
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
        quote_data, // Store the full quote response
    })
}

async fn execute_swap(
    client: &RpcClient,
    wallet: &Keypair,
    quote: &JupiterQuote,
    args: &Args,
    config: &Config,
) -> Result<String, Box<dyn std::error::Error>> {
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
        .await?;
    
    let status = swap_response.status();
    if !status.is_success() {
        let error_text = swap_response.text().await?;
        return Err(format!("Jupiter swap API error: {} - {}", status, error_text).into());
    }
    
    let swap_data: serde_json::Value = swap_response.json().await?;
    
    // Extract the serialized transaction
    let serialized_transaction = swap_data["swapTransaction"]
        .as_str()
        .ok_or("No swapTransaction in response")?;
    
    info!("Received serialized transaction (first 100 chars): {}", 
          &serialized_transaction[..std::cmp::min(100, serialized_transaction.len())]);
    
    // Step 2: Deserialize and sign the transaction (using legacy format)
    let transaction_bytes = base64::engine::general_purpose::STANDARD.decode(serialized_transaction)?;
    
    // Deserialize as regular Transaction (legacy format)
    let mut transaction: Transaction = bincode::deserialize(&transaction_bytes)?;
    
    // Get recent blockhash and update transaction
    let recent_blockhash = client.get_latest_blockhash()?;
    transaction.message.recent_blockhash = recent_blockhash;
    
    // Sign the transaction
    transaction.sign(&[wallet], recent_blockhash);
    
    // Step 3: Send the transaction
    info!("Sending transaction to Solana network...");
    
    let signature = client.send_and_confirm_transaction(&transaction)?;
    
    info!("Transaction sent successfully!");
    info!("Signature: {}", signature);
    
    Ok(signature.to_string())
} 