use crate::models::{TradingSignal, SignalType};
use anyhow::{Result, anyhow};
use serde::{Deserialize, Serialize};
use std::env;
use tracing::{info, warn, error};
use tokio::process::Command;
use std::path::Path;
use solana_sdk::signature::Signer;

#[derive(Debug, Clone)]
pub struct TradingExecutor {
    enable_execution: bool,
    position_size_percentage: f64,
    slippage_tolerance: f64,
    min_confidence_threshold: f64,
    max_concurrent_positions: u32,
    solana_rpc_url: String,
    solana_private_key: String,
    transaction_binary_path: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct WalletBalance {
    pub sol_balance: f64,
    pub usdc_balance: f64,
    pub timestamp: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TransactionResult {
    pub success: bool,
    pub signature: Option<String>,
    pub error: Option<String>,
    pub sol_change: Option<f64>,
    pub usdc_change: Option<f64>,
}

impl TradingExecutor {
    pub fn new() -> Result<Self> {
        // Load .env from project root (two directories up from trading-logic)
        let project_root = std::env::current_dir()?;
        let env_path = if project_root.ends_with("trading-logic") {
            project_root.join("..").join(".env")
        } else {
            project_root.join(".env")
        };
        info!("TradingExecutor: Looking for .env file at: {:?}", env_path);
        dotenv::from_path(&env_path).ok();
        
        let enable_execution = env::var("ENABLE_TRADING_EXECUTION")
            .unwrap_or_else(|_| "false".to_string())
            .parse::<bool>()
            .unwrap_or(false);

        let position_size_percentage = env::var("POSITION_SIZE_PERCENTAGE")
            .unwrap_or_else(|_| "0.9".to_string())
            .parse::<f64>()
            .unwrap_or(0.9);

        let slippage_tolerance = env::var("SLIPPAGE_TOLERANCE")
            .unwrap_or_else(|_| "0.005".to_string())
            .parse::<f64>()
            .unwrap_or(0.005);

        let min_confidence_threshold = env::var("MIN_CONFIDENCE_THRESHOLD")
            .unwrap_or_else(|_| "0.7".to_string())
            .parse::<f64>()
            .unwrap_or(0.7);

        let max_concurrent_positions = env::var("MAX_CONCURRENT_POSITIONS")
            .unwrap_or_else(|_| "1".to_string())
            .parse::<u32>()
            .unwrap_or(1);

        let solana_rpc_url = env::var("SOLANA_RPC_URL")
            .unwrap_or_else(|_| "https://api.mainnet-beta.solana.com".to_string());

        let solana_private_key = env::var("SOLANA_PRIVATE_KEY")
            .map_err(|_| anyhow!("SOLANA_PRIVATE_KEY not found in environment"))?;

        // Determine transaction binary path - try environment variable first, then different locations
        let transaction_binary_path = if let Ok(env_path) = env::var("TRANSACTION_BINARY_PATH") {
            env_path
        } else {
            let possible_paths = vec![
                "../target/debug/transaction",
                "./target/debug/transaction", 
                "target/debug/transaction",
                "solana-trading-bot/target/debug/transaction",
            ];
            
            possible_paths
                .iter()
                .find(|path| Path::new(path).exists())
                .map(|s| s.to_string())
                .unwrap_or_else(|| "../target/debug/transaction".to_string())
        };

        if !Path::new(&transaction_binary_path).exists() {
            warn!("⚠️  Transaction binary not found at expected paths. Trading execution may fail.");
            warn!("   Current path: {}", transaction_binary_path);
            warn!("   Set TRANSACTION_BINARY_PATH environment variable to specify correct path");
            warn!("   Expected locations: ../target/debug/transaction, ./target/debug/transaction, etc.");
        } else {
            info!("✅ Found transaction binary at: {}", transaction_binary_path);
        }

        Ok(Self {
            enable_execution,
            position_size_percentage,
            slippage_tolerance,
            min_confidence_threshold,
            max_concurrent_positions,
            solana_rpc_url,
            solana_private_key,
            transaction_binary_path,
        })
    }

    pub async fn execute_signal(&self, signal: &TradingSignal, sell_quantity: Option<f64>) -> Result<(bool, Option<f64>)> {
        // Check if trading execution is enabled
        if !self.enable_execution {
            info!("🔄 Paper trading mode - signal would be executed: {:?} at ${:.4}", 
                  signal.signal_type, signal.price);
            
            // In paper trading mode, simulate the transaction with dry-run
            let success = self.simulate_trade(signal).await?;
            return Ok((success, None)); // No quantity for paper trading
        }

        // Check confidence threshold
        if signal.confidence < self.min_confidence_threshold {
            warn!("⚠️  Signal confidence ({:.1}%) below threshold ({:.1}%) - skipping execution", 
                  signal.confidence * 100.0, self.min_confidence_threshold * 100.0);
            return Ok((false, None));
        }

        // Get current wallet balance
        let balance = self.get_wallet_balance().await?;
        
        match signal.signal_type {
            SignalType::Buy => {
                let sol_quantity = self.execute_buy_signal(signal, &balance).await?;
                let success = sol_quantity.is_some();
                Ok((success, sol_quantity))
            }
            SignalType::Sell => {
                // For sell signals, we need the exact quantity that was bought
                let success = self.execute_sell_signal(signal, &balance, sell_quantity).await?;
                Ok((success, None)) // No quantity needed for sell
            }
            SignalType::Hold => {
                // Hold signals don't execute trades
                Ok((false, None))
            }
        }
    }

    async fn simulate_trade(&self, signal: &TradingSignal) -> Result<bool> {
        info!("🎭 Simulating trade execution with dry-run...");
        
        let balance = self.get_wallet_balance().await?;
        
        match signal.signal_type {
            SignalType::Buy => {
                let position_size_usdc = balance.usdc_balance * self.position_size_percentage;
                
                if position_size_usdc < 1.0 {
                    warn!("⚠️  Insufficient USDC balance for simulated trade: ${:.2} USDC", balance.usdc_balance);
                    return Ok(false);
                }
                
                let result = self.execute_transaction_command(
                    position_size_usdc,
                    "usdc-to-sol",
                    true, // dry_run = true
                ).await?;
                
                if result.success {
                    info!("✅ BUY trade simulation successful!");
                    info!("📊 Would trade ${:.2} USDC → SOL", position_size_usdc);
                    Ok(true)
                } else {
                    warn!("⚠️  BUY trade simulation failed: {:?}", result.error);
                    Ok(false)
                }
            }
            SignalType::Sell => {
                if balance.sol_balance < 0.001 {
                    warn!("⚠️  Insufficient SOL balance for simulated trade: {:.6} SOL", balance.sol_balance);
                    return Ok(false);
                }
                
                // For sell signals, we should sell the full SOL balance (not just a percentage)
                let position_size_sol = balance.sol_balance;
                
                let result = self.execute_transaction_command(
                    position_size_sol,
                    "sol-to-usdc", 
                    true, // dry_run = true
                ).await?;
                
                if result.success {
                    info!("✅ SELL trade simulation successful!");
                    info!("📊 Would trade {:.6} SOL → USDC", position_size_sol);
                    Ok(true)
                } else {
                    warn!("⚠️  SELL trade simulation failed: {:?}", result.error);
                    Ok(false)
                }
            }
            SignalType::Hold => Ok(false),
        }
    }

    async fn execute_buy_signal(&self, _signal: &TradingSignal, balance: &WalletBalance) -> Result<Option<f64>> {
        info!("🟢 Executing BUY signal...");
        
        // Calculate position size based on USDC balance
        let position_size_usdc = balance.usdc_balance * self.position_size_percentage;
        
        if position_size_usdc < 1.0 {
            warn!("⚠️  Insufficient USDC balance for trade: ${:.2} USDC", balance.usdc_balance);
            return Ok(None);
        }

        info!("💰 Using ${:.2} USDC for trade (${:.2} available)", position_size_usdc, balance.usdc_balance);

        let result = self.execute_transaction_command(
            position_size_usdc,
            "usdc-to-sol",
            false, // dry_run = false
        ).await?;

        if result.success {
            info!("✅ BUY trade executed successfully!");
            if let Some(signature) = &result.signature {
                info!("📊 Transaction signature: {}", signature);
            }
            if let (Some(sol_change), Some(usdc_change)) = (result.sol_change, result.usdc_change) {
                info!("💱 Received {:.6} SOL for ${:.2} USDC", sol_change, usdc_change.abs());
                return Ok(Some(sol_change));
            }
            Ok(None)
        } else {
            let error_msg = result.error.unwrap_or_else(|| "Unknown error".to_string());
            error!("❌ BUY trade failed: {}", error_msg);
            Err(anyhow!("BUY trade execution failed: {}", error_msg))
        }
    }

    async fn execute_sell_signal(&self, _signal: &TradingSignal, balance: &WalletBalance, sell_quantity: Option<f64>) -> Result<bool> {
        info!("🔴 Executing SELL signal...");
        
        // For sell signals, we need to check SOL balance
        if balance.sol_balance < 0.001 {
            warn!("⚠️  Insufficient SOL balance for trade: {:.6} SOL", balance.sol_balance);
            return Ok(false);
        }

        // Use the exact quantity that was bought, not a percentage of current balance
        let position_size_sol = if let Some(quantity) = sell_quantity {
            // Use the exact quantity that was bought
            if quantity > balance.sol_balance {
                warn!("⚠️  Requested sell quantity ({:.6} SOL) exceeds available balance ({:.6} SOL)", 
                      quantity, balance.sol_balance);
                warn!("🔄 Falling back to selling available balance");
                balance.sol_balance
            } else {
                quantity
            }
        } else {
            // Fallback to percentage-based calculation if no quantity provided
            warn!("⚠️  No sell quantity provided, using percentage-based calculation");
            balance.sol_balance * self.position_size_percentage
        };
        
        info!("💰 Using {:.6} SOL for trade ({:.6} available)", position_size_sol, balance.sol_balance);

        let result = self.execute_transaction_command(
            position_size_sol,
            "sol-to-usdc",
            false, // dry_run = false
        ).await?;

        if result.success {
            info!("✅ SELL trade executed successfully!");
            if let Some(signature) = &result.signature {
                info!("📊 Transaction signature: {}", signature);
            }
            if let (Some(sol_change), Some(usdc_change)) = (result.sol_change, result.usdc_change) {
                info!("💱 Traded {:.6} SOL for ${:.2} USDC", sol_change.abs(), usdc_change);
            }
            Ok(true)
        } else {
            let error_msg = result.error.unwrap_or_else(|| "Unknown error".to_string());
            error!("❌ SELL trade failed: {}", error_msg);
            Err(anyhow!("SELL trade execution failed: {}", error_msg))
        }
    }

    async fn execute_transaction_command(
        &self,
        amount: f64,
        direction: &str,
        dry_run: bool,
    ) -> Result<TransactionResult> {
        let slippage_bps = (self.slippage_tolerance * 10000.0) as u32;
        
        let mut cmd = Command::new(&self.transaction_binary_path);
        cmd.env("SOLANA_RPC_URL", &self.solana_rpc_url)
           .env("SOLANA_PRIVATE_KEY", &self.solana_private_key);

        // The transaction binary uses --amount-usdc parameter for both directions:
        // - For "usdc-to-sol": amount represents USDC to spend
        // - For "sol-to-usdc": amount represents SOL to sell (confusing naming but correct behavior)
        match direction {
            "usdc-to-sol" => {
                cmd.arg("--amount-usdc")
                   .arg(&amount.to_string());
            }
            "sol-to-usdc" => {
                // Amount here is SOL amount to sell, but parameter name is still --amount-usdc
                // This is how the transaction binary expects it (see jupiter.rs line ~24)
                cmd.arg("--amount-usdc")
                   .arg(&amount.to_string());
            }
            _ => return Err(anyhow!("Invalid direction: {}", direction)),
        }

        cmd.arg("--direction")
           .arg(direction)
           .arg("--slippage-bps")
           .arg(&slippage_bps.to_string());

        if dry_run {
            cmd.arg("--dry-run");
        }

        info!("🔧 Executing command: {} {:?}", self.transaction_binary_path, cmd.as_std().get_args().collect::<Vec<_>>());

        let output = cmd.output().await?;
        
        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            info!("📊 Transaction output: {}", stdout);
            
            // Parse the output to extract transaction details
            let transaction_result = self.parse_transaction_output(&stdout, dry_run)?;
            Ok(transaction_result)
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            error!("❌ Transaction command failed: {}", stderr);
            
            Ok(TransactionResult {
                success: false,
                signature: None,
                error: Some(stderr.to_string()),
                sol_change: None,
                usdc_change: None,
            })
        }
    }

    fn parse_transaction_output(&self, output: &str, dry_run: bool) -> Result<TransactionResult> {
        let mut signature = None;
        let mut sol_change = None;
        let mut usdc_change = None;
        
        info!("🔍 Parsing transaction output for SOL/USDC changes...");
        
        for line in output.lines() {
            let trimmed_line = line.trim();
            info!("🔍 Parsing line: '{}'", trimmed_line);
            
            if trimmed_line.contains("Transaction Signature:") || trimmed_line.contains("Signature:") {
                signature = trimmed_line.split(':').nth(1).map(|s| s.trim().to_string());
                info!("📝 Found signature: {:?}", signature);
            } else if trimmed_line.contains("SOL:") && trimmed_line.contains("(received)") {
                // Look for pattern like "SOL: 0.123456 SOL (received)"
                if let Some(change_str) = trimmed_line.split("SOL:").nth(1) {
                    // Extract the number before "SOL"
                    let parts: Vec<&str> = change_str.split_whitespace().collect();
                    if let Some(num_str) = parts.first() {
                        let parsed = num_str.parse::<f64>();
                        sol_change = parsed.ok();
                        info!("🟢 Found SOL change: {:?} (parsed from '{}')", sol_change, num_str);
                    }
                }
            } else if trimmed_line.contains("USDC:") && (trimmed_line.contains("(spent)") || trimmed_line.contains("(received)")) {
                // Look for pattern like "USDC: 100.00 USDC (spent)"
                if let Some(change_str) = trimmed_line.split("USDC:").nth(1) {
                    // Extract the number before "USDC"
                    let parts: Vec<&str> = change_str.split_whitespace().collect();
                    if let Some(num_str) = parts.first() {
                        let mut change = num_str.parse::<f64>().unwrap_or(0.0);
                        if trimmed_line.contains("(spent)") {
                            change = -change; // Make spent amounts negative
                        }
                        usdc_change = Some(change);
                        info!("🟢 Found USDC change: {:?} (parsed from '{}')", usdc_change, num_str);
                    }
                }
            }
        }
        
        info!("📊 Parsing results - SOL: {:?}, USDC: {:?}, Signature: {:?}", sol_change, usdc_change, signature);
        
        Ok(TransactionResult {
            success: true,
            signature: if dry_run { None } else { signature }, // No signature in dry-run mode
            error: None,
            sol_change,
            usdc_change,
        })
    }

    async fn get_wallet_balance(&self) -> Result<WalletBalance> {
        // Use the transaction binary with a dry-run to get current balances
        // This is a workaround since we don't have a dedicated balance command
        
        let mut cmd = Command::new(&self.transaction_binary_path);
        cmd.env("SOLANA_RPC_URL", &self.solana_rpc_url)
           .env("SOLANA_PRIVATE_KEY", &self.solana_private_key)
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
                let trimmed_line = line.trim();
                if trimmed_line.contains("SOL:") && trimmed_line.contains("SOL") {
                    // Look for lines like "SOL: 1.234567 SOL"
                    if let Some(balance_part) = trimmed_line.split("SOL:").nth(1) {
                        // Extract the number before "SOL"
                        let parts: Vec<&str> = balance_part.split_whitespace().collect();
                        if let Some(number_part) = parts.first() {
                            sol_balance = number_part.parse::<f64>().unwrap_or(0.0);
                        }
                    }
                } else if trimmed_line.contains("USDC:") && trimmed_line.contains("USDC") {
                    // Look for lines like "USDC: 100.00 USDC"  
                    if let Some(balance_part) = trimmed_line.split("USDC:").nth(1) {
                        // Extract the number before "USDC"
                        let parts: Vec<&str> = balance_part.split_whitespace().collect();
                        if let Some(number_part) = parts.first() {
                            usdc_balance = number_part.parse::<f64>().unwrap_or(0.0);
                        }
                    }
                }
            }

            Ok(WalletBalance {
                sol_balance,
                usdc_balance,
                timestamp: chrono::Utc::now().to_rfc3339(),
            })
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            Err(anyhow!("Failed to get wallet balance: {}", stderr))
        }
    }

    pub fn is_trading_enabled(&self) -> bool {
        self.enable_execution
    }

    pub fn get_position_size_percentage(&self) -> f64 {
        self.position_size_percentage
    }

    pub fn get_slippage_tolerance(&self) -> f64 {
        self.slippage_tolerance
    }

    pub fn get_min_confidence_threshold(&self) -> f64 {
        self.min_confidence_threshold
    }

    pub fn get_wallet_address(&self) -> Result<String> {
        // Convert private key to wallet address
        let private_key = self.solana_private_key.trim();
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

    async fn get_last_transaction_sol_quantity(&self) -> Result<Option<f64>> {
        // This method would extract the SOL quantity from the last transaction result
        // For now, we'll use a simple approach by checking the transaction output
        // In a real implementation, you'd store the last transaction result
        
        // Since we don't have persistent storage of transaction results,
        // we'll estimate the quantity based on the position size percentage
        // This is a temporary workaround - ideally you'd store the actual transaction result
        
        let balance = self.get_wallet_balance().await?;
        let position_size_usdc = balance.usdc_balance * self.position_size_percentage;
        
        // Estimate SOL quantity based on current price
        // This is approximate - the actual quantity depends on the exact price at execution
        let estimated_sol_quantity = position_size_usdc / 150.0; // Approximate SOL price
        
        Ok(Some(estimated_sol_quantity))
    }
} 