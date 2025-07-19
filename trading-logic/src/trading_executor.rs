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
    wallet_index: usize,
    wallet_name: String,
    last_transaction_result: Option<TransactionResult>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct WalletBalance {
    pub sol_balance: f64,
    pub usdc_balance: f64,
    pub timestamp: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionResult {
    pub success: bool,
    pub signature: Option<String>,
    pub error: Option<String>,
    pub sol_change: Option<f64>,
    pub usdc_change: Option<f64>,
    pub execution_price: Option<f64>,
}

impl TradingExecutor {
    pub fn new() -> Result<Self> {
        Self::new_with_wallet(0, "Default".to_string(), None)
    }

    pub fn new_with_wallet(wallet_index: usize, wallet_name: String, private_key: Option<String>) -> Result<Self> {
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

        // Use provided private key or fallback to environment variable
        let solana_private_key = if let Some(key) = private_key {
            key
        } else {
            env::var("SOLANA_PRIVATE_KEY")
                .map_err(|_| anyhow!("SOLANA_PRIVATE_KEY not found in environment"))?
        };

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
            wallet_index,
            wallet_name,
            last_transaction_result: None,
        })
    }

    pub fn get_wallet_index(&self) -> usize {
        self.wallet_index
    }

    pub fn get_wallet_name(&self) -> &str {
        &self.wallet_name
    }

    pub async fn execute_signal(&self, signal: &TradingSignal, sell_quantity: Option<f64>) -> Result<(bool, Option<f64>, Option<f64>, Option<f64>)> {
        // Check if trading execution is enabled
        if !self.enable_execution {
            info!("🔄 Paper trading mode - signal would be executed: {:?} at ${:.4}", 
                  signal.signal_type, signal.price);
            
            // In paper trading mode, simulate the transaction with dry-run
            let success = self.simulate_trade(signal).await?;
            return Ok((success, None, None, None)); // No quantity, execution price, or USDC change for paper trading
        }

        // Check confidence threshold
        if signal.confidence < self.min_confidence_threshold {
            warn!("⚠️  Signal confidence ({:.1}%) below threshold ({:.1}%) - skipping execution", 
                  signal.confidence * 100.0, self.min_confidence_threshold * 100.0);
            return Ok((false, None, None, None));
        }

        // Get current wallet balance
        let balance = self.get_wallet_balance().await?;
        
        match signal.signal_type {
            SignalType::Buy => {
                let (sol_quantity, execution_price, usdc_change) = self.execute_buy_signal(signal, &balance).await?;
                let success = sol_quantity.is_some();
                Ok((success, sol_quantity, execution_price, usdc_change))
            }
            SignalType::Sell => {
                // For sell signals, we need the exact quantity that was bought
                let (success, execution_price, usdc_change) = self.execute_sell_signal(signal, &balance, sell_quantity).await?;
                Ok((success, None, execution_price, usdc_change)) // No quantity needed for sell
            }
            SignalType::Hold => {
                // Hold signals don't execute trades
                Ok((false, None, None, None))
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

    async fn execute_buy_signal(&self, _signal: &TradingSignal, balance: &WalletBalance) -> Result<(Option<f64>, Option<f64>, Option<f64>)> {
        info!("🟢 Executing BUY signal...");
        
        // Get precise balance before trade for accurate P&L tracking
        let balance_before = self.get_wallet_balance().await?;
        info!("💰 Balance before BUY: ${:.2} USDC, {:.6} SOL", 
              balance_before.usdc_balance, balance_before.sol_balance);
        
        // Calculate position size based on actual current USDC balance
        let position_size_usdc = balance_before.usdc_balance * self.position_size_percentage;
        
        if position_size_usdc < 1.0 {
            warn!("⚠️  Insufficient USDC balance for trade: ${:.2} USDC", balance_before.usdc_balance);
            return Ok((None, None, None));
        }

        info!("💰 Using ${:.2} USDC for trade (${:.2} available)", position_size_usdc, balance_before.usdc_balance);

        let result = self.execute_transaction_command(
            position_size_usdc,
            "usdc-to-sol",
            false, // dry_run = false
        ).await?;

        if result.success {
            // Get precise balance after trade for accurate P&L tracking
            let balance_after = self.get_wallet_balance().await?;
            info!("💰 Balance after BUY: ${:.2} USDC, {:.6} SOL", 
                  balance_after.usdc_balance, balance_after.sol_balance);
            
            // Calculate ACTUAL wallet balance changes (this is the real P&L)
            let actual_usdc_change = balance_after.usdc_balance - balance_before.usdc_balance;
            let actual_sol_change = balance_after.sol_balance - balance_before.sol_balance;
            
            info!("✅ BUY trade executed successfully!");
            info!("📊 ACTUAL wallet balance changes: USDC: ${:.2}, SOL: {:.6}", 
                  actual_usdc_change, actual_sol_change);
            info!("💰 Real USDC P&L: Spent ${:.2} USDC (including all fees)", actual_usdc_change.abs());
            
            if let Some(signature) = &result.signature {
                info!("📊 Transaction signature: {}", signature);
            }
            
            // Return ACTUAL balance changes, not transaction parsing
            Ok((Some(actual_sol_change), result.execution_price, Some(actual_usdc_change)))
        } else {
            let error_msg = result.error.unwrap_or_else(|| "Unknown error".to_string());
            error!("❌ BUY trade failed: {}", error_msg);
            Err(anyhow!("BUY trade execution failed: {}", error_msg))
        }
    }

    async fn execute_sell_signal(&self, _signal: &TradingSignal, balance: &WalletBalance, sell_quantity: Option<f64>) -> Result<(bool, Option<f64>, Option<f64>)> {
        info!("🔴 Executing SELL signal...");
        
        // Get precise balance before trade for accurate P&L tracking
        let balance_before = self.get_wallet_balance().await?;
        info!("💰 Balance before SELL: ${:.2} USDC, {:.6} SOL", 
              balance_before.usdc_balance, balance_before.sol_balance);
        
        // For sell signals, we need to check SOL balance
        if balance_before.sol_balance < 0.001 {
            warn!("⚠️  Insufficient SOL balance for trade: {:.6} SOL", balance_before.sol_balance);
            return Ok((false, None, None));
        }

        // Use the exact quantity that was bought, not a percentage of current balance
        let position_size_sol = if let Some(quantity) = sell_quantity {
            // Use the exact quantity that was bought
            if quantity > balance_before.sol_balance {
                warn!("⚠️  Requested sell quantity ({:.6} SOL) exceeds available balance ({:.6} SOL)", 
                      quantity, balance_before.sol_balance);
                warn!("🔄 Falling back to selling available balance");
                balance_before.sol_balance
            } else {
                quantity
            }
        } else {
            // Fallback to percentage-based calculation if no quantity provided
            warn!("⚠️  No sell quantity provided, using percentage-based calculation");
            balance_before.sol_balance * self.position_size_percentage
        };
        
        info!("💰 Using {:.6} SOL for trade ({:.6} available)", position_size_sol, balance_before.sol_balance);

        let result = self.execute_transaction_command(
            position_size_sol,
            "sol-to-usdc",
            false, // dry_run = false
        ).await?;

        if result.success {
            // Get precise balance after trade for accurate P&L tracking
            let balance_after = self.get_wallet_balance().await?;
            info!("💰 Balance after SELL: ${:.2} USDC, {:.6} SOL", 
                  balance_after.usdc_balance, balance_after.sol_balance);
            
            // Calculate ACTUAL wallet balance changes (this is the real P&L)
            let actual_usdc_change = balance_after.usdc_balance - balance_before.usdc_balance;
            let actual_sol_change = balance_after.sol_balance - balance_before.sol_balance;
            
            info!("✅ SELL trade executed successfully!");
            info!("📊 ACTUAL wallet balance changes: USDC: ${:.2}, SOL: {:.6}", 
                  actual_usdc_change, actual_sol_change);
            info!("💰 Real USDC P&L: Received ${:.2} USDC (including all fees)", actual_usdc_change);
            
            if let Some(signature) = &result.signature {
                info!("📊 Transaction signature: {}", signature);
            }
            
            // Return ACTUAL balance changes, not transaction parsing
            Ok((true, result.execution_price, Some(actual_usdc_change)))
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
        let max_retries = 3;
        let mut last_error = None;

        for attempt in 1..=max_retries {
            info!("🔄 Transaction attempt {}/{} for {} {}", attempt, max_retries, amount, direction);
            
            let result = self.execute_single_transaction_attempt(amount, direction, dry_run).await;
            
            match result {
                Ok(transaction_result) => {
                    if transaction_result.success {
                        if attempt > 1 {
                            info!("✅ Transaction succeeded on attempt {}/{}", attempt, max_retries);
                        }
                        return Ok(transaction_result);
                    } else {
                        // Transaction failed, check if we should retry
                        if let Some(ref error) = transaction_result.error {
                            if self.should_retry_transaction(error) && attempt < max_retries {
                                let delay = 2u64.pow(attempt - 1); // Exponential backoff: 1s, 2s, 4s
                                warn!("⚠️  Transaction failed (attempt {}/{}), retrying in {} seconds... Error: {}", 
                                      attempt, max_retries, delay, error);
                                tokio::time::sleep(tokio::time::Duration::from_secs(delay)).await;
                                last_error = Some(error.clone());
                                continue;
                            }
                        }
                        return Ok(transaction_result);
                    }
                }
                Err(e) => {
                    if attempt < max_retries {
                        let delay = 2u64.pow(attempt - 1);
                        warn!("⚠️  Transaction execution error (attempt {}/{}), retrying in {} seconds... Error: {}", 
                              attempt, max_retries, delay, e);
                        tokio::time::sleep(tokio::time::Duration::from_secs(delay)).await;
                        last_error = Some(e.to_string());
                        continue;
                    } else {
                        return Err(e);
                    }
                }
            }
        }

        // All retries failed
        error!("❌ All {} transaction attempts failed", max_retries);
        Ok(TransactionResult {
            success: false,
            signature: None,
            error: last_error,
            sol_change: None,
            usdc_change: None,
            execution_price: None,
        })
    }

    fn should_retry_transaction(&self, error: &str) -> bool {
        // Retry on common Jupiter/Solana errors that are often temporary
        let retryable_errors = [
            "Transaction simulation failed",
            "custom program error: 0x9ca", // Jupiter slippage error
            "RPC response error",
            "Network error",
            "Timeout",
            "Connection refused",
            "Service unavailable",
            "Internal server error",
        ];
        
        retryable_errors.iter().any(|&retryable| error.contains(retryable))
    }

    async fn execute_single_transaction_attempt(
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
                execution_price: None,
            })
        }
    }

    fn parse_transaction_output(&self, output: &str, dry_run: bool) -> Result<TransactionResult> {
        let mut signature = None;
        let mut sol_change = None;
        let mut usdc_change = None;
        let mut execution_price = None;
        
        info!("🔍 Parsing transaction output for SOL/USDC changes and execution price...");
        
        for line in output.lines() {
            info!("🔍 Parsing line: '{}'", line.trim());
            
            if line.contains("Transaction Signature:") || line.contains("Signature:") {
                signature = line.split(':').nth(1).map(|s| s.trim().to_string());
                info!("📝 Found signature: {:?}", signature);
            } else if line.contains("Jupiter Execution Price:") {
                // Look for pattern like "  Jupiter Execution Price: $150.1234 per SOL"
                if let Some(price_str) = line.split("$").nth(1) {
                    if let Some(num_str) = price_str.split(" per SOL").next() {
                        let parsed = num_str.trim().parse::<f64>();
                        execution_price = parsed.ok();
                        info!("💰 Found execution price: {:?} (parsed from '{}')", execution_price, num_str.trim());
                    }
                }
            } else if line.contains("SOL:") && (line.contains("(received)") || line.contains("(spent)")) {
                // Look for pattern like "  SOL: 0.123456 SOL (received)" or "  SOL: 0.123456 SOL (spent)"
                if let Some(change_str) = line.split("SOL:").nth(1) {
                    if let Some(num_str) = change_str.split("SOL").next() {
                        let mut change = num_str.trim().parse::<f64>().unwrap_or(0.0);
                        if line.contains("(spent)") {
                            change = -change; // Make spent amounts negative
                        }
                        sol_change = Some(change);
                        info!("🟢 Found SOL change: {:?} (parsed from '{}')", sol_change, num_str.trim());
                    }
                }
            } else if line.contains("USDC:") && (line.contains("(spent)") || line.contains("(received)")) {
                // Look for pattern like "  USDC: 100.00 USDC (spent)" or "  USDC: 100.00 USDC (received)"
                if let Some(change_str) = line.split("USDC:").nth(1) {
                    if let Some(num_str) = change_str.split("USDC").next() {
                        let mut change = num_str.trim().parse::<f64>().unwrap_or(0.0);
                        if line.contains("(spent)") {
                            change = -change; // Make spent amounts negative
                        }
                        usdc_change = Some(change);
                        info!("🟢 Found USDC change: {:?} (parsed from '{}')", usdc_change, num_str.trim());
                    }
                }
            }
        }
        
        info!("📊 Parsing results - SOL: {:?}, USDC: {:?}, Execution Price: {:?}, Signature: {:?}", 
              sol_change, usdc_change, execution_price, signature);
        
        Ok(TransactionResult {
            success: true,
            signature: if dry_run { None } else { signature }, // No signature in dry-run mode
            error: None,
            sol_change,
            usdc_change,
            execution_price,
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

    // Store the last transaction result for USDC PnL tracking
    pub fn store_transaction_result(&mut self, result: TransactionResult) {
        self.last_transaction_result = Some(result);
    }

    // Get the last transaction result for USDC PnL calculation
    pub fn get_last_transaction_result(&self) -> Option<&TransactionResult> {
        self.last_transaction_result.as_ref()
    }

    // Get USDC change from last transaction (for PnL tracking)
    pub fn get_last_usdc_change(&self) -> Option<f64> {
        self.last_transaction_result.as_ref()?.usdc_change
    }

    // Get SOL change from last transaction (for quantity tracking)
    pub fn get_last_sol_change(&self) -> Option<f64> {
        self.last_transaction_result.as_ref()?.sol_change
    }

    async fn get_last_transaction_sol_quantity(&self) -> Result<Option<f64>> {
        // Use actual transaction result if available
        if let Some(result) = &self.last_transaction_result {
            if let Some(sol_change) = result.sol_change {
                return Ok(Some(sol_change.abs())); // Return absolute value for quantity
            }
        }
        
        // Fallback to estimation if no transaction result stored
        let balance = self.get_wallet_balance().await?;
        let position_size_usdc = balance.usdc_balance * self.position_size_percentage;
        let estimated_sol_quantity = position_size_usdc / 150.0; // Approximate SOL price
        Ok(Some(estimated_sol_quantity))
    }
} 