use crate::models::{TradingSignal, SignalType};
use anyhow::{Result, anyhow};
use serde::{Deserialize, Serialize};
use std::process::Command;
use std::env;
use tracing::{info, warn, error};

#[derive(Debug, Clone)]
pub struct TradingExecutor {
    enable_execution: bool,
    position_size_percentage: f64,
    slippage_tolerance: f64,
    min_confidence_threshold: f64,
    max_concurrent_positions: u32,
    solana_rpc_url: String,
    solana_private_key: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct WalletBalance {
    pub sol_balance: f64,
    pub usdc_balance: f64,
    pub timestamp: String,
}

impl TradingExecutor {
    pub fn new() -> Result<Self> {
        // Load .env from project root (parent directory of trading-logic)
        let project_root = std::env::current_dir()?.join("..");
        let env_path = project_root.join(".env");
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

        Ok(Self {
            enable_execution,
            position_size_percentage,
            slippage_tolerance,
            min_confidence_threshold,
            max_concurrent_positions,
            solana_rpc_url,
            solana_private_key,
        })
    }

    pub async fn execute_signal(&self, signal: &TradingSignal) -> Result<bool> {
        // Check if trading execution is enabled
        if !self.enable_execution {
            info!("🔄 Paper trading mode - signal would be executed: {:?} at ${:.4}", 
                  signal.signal_type, signal.price);
            return Ok(true);
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

    async fn execute_buy_signal(&self, signal: &TradingSignal, balance: &WalletBalance) -> Result<bool> {
        info!("🟢 Executing BUY signal...");
        
        // Calculate position size based on USDC balance
        let position_size_usdc = balance.usdc_balance * self.position_size_percentage;
        
        if position_size_usdc < 1.0 {
            warn!("⚠️  Insufficient USDC balance for trade: ${:.2} USDC", balance.usdc_balance);
            return Ok(false);
        }

        info!("💰 Using ${:.2} USDC for trade (${:.2} available)", position_size_usdc, balance.usdc_balance);

        // Execute USDC to SOL swap
        let slippage_bps = (self.slippage_tolerance * 10000.0) as u32;
        
        let result = Command::new("../target/debug/transaction")
            .arg("--amount-usdc")
            .arg(&position_size_usdc.to_string())
            .arg("--direction")
            .arg("usdc-to-sol")
            .arg("--slippage-bps")
            .arg(&slippage_bps.to_string())
            .env("SOLANA_RPC_URL", &self.solana_rpc_url)
            .env("SOLANA_PRIVATE_KEY", &self.solana_private_key)
            .output()?;

        if result.status.success() {
            let output = String::from_utf8_lossy(&result.stdout);
            info!("✅ BUY trade executed successfully!");
            info!("📊 Trade output: {}", output);
            Ok(true)
        } else {
            let error = String::from_utf8_lossy(&result.stderr);
            error!("❌ BUY trade failed: {}", error);
            Ok(false)
        }
    }

    async fn execute_sell_signal(&self, signal: &TradingSignal, balance: &WalletBalance) -> Result<bool> {
        info!("🔴 Executing SELL signal...");
        
        // For sell signals, we need to check SOL balance
        if balance.sol_balance < 0.001 {
            warn!("⚠️  Insufficient SOL balance for trade: {:.6} SOL", balance.sol_balance);
            return Ok(false);
        }

        // Calculate position size based on SOL balance
        let position_size_sol = balance.sol_balance * self.position_size_percentage;
        
        info!("💰 Using {:.6} SOL for trade ({:.6} available)", position_size_sol, balance.sol_balance);

        // Execute SOL to USDC swap
        let slippage_bps = (self.slippage_tolerance * 10000.0) as u32;
        
        let result = Command::new("../target/debug/transaction")
            .arg("--amount-usdc")
            .arg(&position_size_sol.to_string())
            .arg("--direction")
            .arg("sol-to-usdc")
            .arg("--slippage-bps")
            .arg(&slippage_bps.to_string())
            .env("SOLANA_RPC_URL", &self.solana_rpc_url)
            .env("SOLANA_PRIVATE_KEY", &self.solana_private_key)
            .output()?;

        if result.status.success() {
            let output = String::from_utf8_lossy(&result.stdout);
            info!("✅ SELL trade executed successfully!");
            info!("📊 Trade output: {}", output);
            Ok(true)
        } else {
            let error = String::from_utf8_lossy(&result.stderr);
            error!("❌ SELL trade failed: {}", error);
            Ok(false)
        }
    }

    async fn get_wallet_balance(&self) -> Result<WalletBalance> {
        let result = Command::new("../target/debug/solana-trading-bot")
            .arg("balance")
            .env("SOLANA_RPC_URL", &self.solana_rpc_url)
            .env("SOLANA_PRIVATE_KEY", &self.solana_private_key)
            .output()?;

        if result.status.success() {
            let output = String::from_utf8_lossy(&result.stdout);
            
            // Parse the balance output (assuming it's in a readable format)
            // This is a simplified parser - you might need to adjust based on actual output format
            let lines: Vec<&str> = output.lines().collect();
            
            let mut sol_balance = 0.0;
            let mut usdc_balance = 0.0;
            
            for line in lines {
                if line.contains("SOL Balance:") {
                    if let Some(balance_str) = line.split("SOL Balance:").nth(1) {
                        sol_balance = balance_str.trim().parse::<f64>().unwrap_or(0.0);
                    }
                } else if line.contains("USDC Balance:") {
                    if let Some(balance_str) = line.split("USDC Balance:").nth(1) {
                        usdc_balance = balance_str.trim().parse::<f64>().unwrap_or(0.0);
                    }
                }
            }

            Ok(WalletBalance {
                sol_balance,
                usdc_balance,
                timestamp: chrono::Utc::now().to_rfc3339(),
            })
        } else {
            let error = String::from_utf8_lossy(&result.stderr);
            Err(anyhow!("Failed to get wallet balance: {}", error))
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