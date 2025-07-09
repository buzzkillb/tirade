use crate::models::{TradingSignal, SignalType};
use anyhow::{Result, anyhow};
use serde::{Deserialize, Serialize};
use std::env;
use tracing::{info, warn, error};
use tokio::process::Command;
use std::path::Path;

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
        
        // Convert JSON array format to base58 if needed
        let solana_private_key = if solana_private_key.starts_with('[') {
            // Parse JSON array and convert to base58
            let key_bytes: Vec<u8> = serde_json::from_str(&solana_private_key)
                .map_err(|e| anyhow!("Invalid SOLANA_PRIVATE_KEY format: {}", e))?;
            bs58::encode(key_bytes).into_string()
        } else {
            solana_private_key
        };

        // Determine transaction binary path - try different locations
        let possible_paths = vec![
            "../target/debug/transaction",
            "./target/debug/transaction", 
            "target/debug/transaction",
            "solana-trading-bot/target/debug/transaction",
        ];
        
        let transaction_binary_path = possible_paths
            .iter()
            .find(|path| Path::new(path).exists())
            .map(|s| s.to_string())
            .unwrap_or_else(|| "../target/debug/transaction".to_string());

        if !Path::new(&transaction_binary_path).exists() {
            warn!("⚠️  Transaction binary not found at expected paths. Trading execution may fail.");
            warn!("   Expected locations: {:?}", possible_paths);
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

    pub async fn execute_signal(&self, signal: &TradingSignal) -> Result<bool> {
        // Check if trading execution is enabled
        if !self.enable_execution {
            info!("🔄 Paper trading mode - signal would be executed: {:?} at ${:.4}", 
                  signal.signal_type, signal.price);
            
            // In paper trading mode, simulate the transaction with dry-run
            return self.simulate_trade(signal).await;
        }

        // Check confidence threshold
        if signal.confidence < self.min_confidence_threshold {
            warn!("⚠️  Signal confidence ({:.1}%) below threshold ({:.1}%) - skipping execution", 
                  signal.confidence * 100.0, self.min_confidence_threshold * 100.0);
            return Ok(false);
        }

        // Get current wallet balance
        let balance = self.get_wallet_balance().await?;
        
        match signal.signal_type {
            SignalType::Buy => {
                self.execute_buy_signal(signal, &balance).await
            }
            SignalType::Sell => {
                self.execute_sell_signal(signal, &balance).await
            }
            SignalType::Hold => {
                // Hold signals don't execute trades
                Ok(false)
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
                
                let position_size_sol = balance.sol_balance * self.position_size_percentage;
                
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

    async fn execute_buy_signal(&self, _signal: &TradingSignal, balance: &WalletBalance) -> Result<bool> {
        info!("🟢 Executing BUY signal...");
        
        // Calculate position size based on USDC balance
        let position_size_usdc = balance.usdc_balance * self.position_size_percentage;
        
        if position_size_usdc < 1.0 {
            warn!("⚠️  Insufficient USDC balance for trade: ${:.2} USDC", balance.usdc_balance);
            return Ok(false);
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
            }
            Ok(true)
        } else {
            let error_msg = result.error.unwrap_or_else(|| "Unknown error".to_string());
            error!("❌ BUY trade failed: {}", error_msg);
            Err(anyhow!("BUY trade execution failed: {}", error_msg))
        }
    }

    async fn execute_sell_signal(&self, _signal: &TradingSignal, balance: &WalletBalance) -> Result<bool> {
        info!("🔴 Executing SELL signal...");
        
        // For sell signals, we need to check SOL balance
        if balance.sol_balance < 0.001 {
            warn!("⚠️  Insufficient SOL balance for trade: {:.6} SOL", balance.sol_balance);
            return Ok(false);
        }

        // Calculate position size based on SOL balance
        let position_size_sol = balance.sol_balance * self.position_size_percentage;
        
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
        
        for line in output.lines() {
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
} 