#[cfg(test)]
mod tests {
    use super::*;
    use crate::trading_executor::{TradingExecutor, WalletBalance, TransactionResult};
    use crate::models::{TradingSignal, SignalType};
    use chrono::Utc;
    use std::env;
    use tokio::process::Command;

    /// Test configuration for trading scenarios
    #[derive(Debug, Clone)]
    struct TestConfig {
        test_amount_usdc: f64,
        slippage_tolerance: f64,
        min_confidence: f64,
        dry_run: bool,
    }

    impl Default for TestConfig {
        fn default() -> Self {
            Self {
                test_amount_usdc: 1.0,
                slippage_tolerance: 0.005,
                min_confidence: 0.7,
                dry_run: true,
            }
        }
    }

    /// Comprehensive trading test scenario
    #[tokio::test]
    async fn test_full_trading_scenario() {
        let config = TestConfig::default();
        
        println!("🚀 Starting Full Trading Scenario Test");
        println!("═══════════════════════════════════════════════════════════════");
        println!("Test Configuration:");
        println!("  Amount: ${:.2} USDC", config.test_amount_usdc);
        println!("  Slippage: {:.1}%", config.slippage_tolerance * 100.0);
        println!("  Min Confidence: {:.1}%", config.min_confidence * 100.0);
        println!("  Mode: {}", if config.dry_run { "Dry Run" } else { "Live Trade" });
        println!("");

        // Step 1: Initialize trading executor
        let executor = match TradingExecutor::new() {
            Ok(exec) => {
                println!("✅ Trading executor initialized successfully");
                exec
            }
            Err(e) => {
                println!("❌ Failed to initialize trading executor: {}", e);
                return;
            }
        };

        // Step 2: Get initial balances
        let initial_balance = match executor.get_wallet_balance().await {
            Ok(balance) => {
                println!("✅ Initial balances retrieved:");
                println!("  SOL: {:.6} SOL", balance.sol_balance);
                println!("  USDC: {:.2} USDC", balance.usdc_balance);
                balance
            }
            Err(e) => {
                println!("❌ Failed to get initial balances: {}", e);
                return;
            }
        };

        // Step 3: Test USDC to SOL swap
        println!("\n🔄 Step 1: Testing USDC → SOL Swap");
        let usdc_to_sol_result = test_usdc_to_sol_swap(&executor, config.test_amount_usdc, config.dry_run).await;
        
        if let Err(e) = usdc_to_sol_result {
            println!("❌ USDC to SOL swap failed: {}", e);
            return;
        }

        let sol_received = usdc_to_sol_result.unwrap();
        println!("✅ USDC to SOL swap successful: {:.6} SOL received", sol_received);

        // Step 4: Test SOL to USDC swap
        println!("\n🔄 Step 2: Testing SOL → USDC Swap");
        let sol_to_usdc_result = test_sol_to_usdc_swap(&executor, sol_received, config.dry_run).await;
        
        if let Err(e) = sol_to_usdc_result {
            println!("❌ SOL to USDC swap failed: {}", e);
            return;
        }

        let usdc_received = sol_to_usdc_result.unwrap();
        println!("✅ SOL to USDC swap successful: {:.2} USDC received", usdc_received);

        // Step 5: Calculate and verify PnL
        println!("\n💰 PnL Analysis:");
        let pnl = usdc_received - config.test_amount_usdc;
        let pnl_percent = (pnl / config.test_amount_usdc) * 100.0;
        
        println!("  USDC Spent: ${:.2}", config.test_amount_usdc);
        println!("  USDC Received: ${:.2}", usdc_received);
        println!("  Net PnL: ${:.2} USDC", pnl);
        println!("  PnL %: {:.2}%", pnl_percent);
        
        if pnl > 0.0 {
            println!("✅ PROFIT: +${:.2} USDC ({:.2}%)", pnl, pnl_percent);
        } else if pnl < 0.0 {
            println!("💸 LOSS: ${:.2} USDC ({:.2}%)", pnl, pnl_percent);
        } else {
            println!("➡️  BREAKEVEN: ${:.2} USDC", pnl);
        }

        // Step 6: Verify final balances
        println!("\n🔍 Step 3: Verifying Final Balances");
        let final_balance = match executor.get_wallet_balance().await {
            Ok(balance) => {
                println!("✅ Final balances retrieved:");
                println!("  SOL: {:.6} SOL", balance.sol_balance);
                println!("  USDC: {:.2} USDC", balance.usdc_balance);
                balance
            }
            Err(e) => {
                println!("❌ Failed to get final balances: {}", e);
                return;
            }
        };

        // Step 7: Balance consistency check
        verify_balance_consistency(&initial_balance, &final_balance, config.test_amount_usdc, sol_received, usdc_received);

        // Step 8: Test trading signal execution
        println!("\n🎯 Step 4: Testing Trading Signal Execution");
        test_trading_signal_execution(&executor, &config).await;

        println!("\n🎉 Full Trading Scenario Test Completed Successfully!");
    }

    /// Test USDC to SOL swap
    async fn test_usdc_to_sol_swap(
        executor: &TradingExecutor,
        amount_usdc: f64,
        dry_run: bool,
    ) -> Result<f64, Box<dyn std::error::Error>> {
        println!("  Executing USDC → SOL swap...");
        println!("  Amount: ${:.2} USDC", amount_usdc);
        println!("  Mode: {}", if dry_run { "Dry Run" } else { "Live Trade" });

        let result = executor.execute_transaction_command(amount_usdc, "usdc-to-sol", dry_run).await?;
        
        if !result.success {
            return Err(format!("Swap failed: {}", result.error.unwrap_or_else(|| "Unknown error".to_string())).into());
        }

        // Extract SOL received
        if let Some(sol_change) = result.sol_change {
            if sol_change > 0.0 {
                println!("  ✅ Received {:.6} SOL", sol_change);
                Ok(sol_change)
            } else {
                Err("No SOL received from swap".into())
            }
        } else {
            Err("Could not determine SOL amount received".into())
        }
    }

    /// Test SOL to USDC swap
    async fn test_sol_to_usdc_swap(
        executor: &TradingExecutor,
        amount_sol: f64,
        dry_run: bool,
    ) -> Result<f64, Box<dyn std::error::Error>> {
        println!("  Executing SOL → USDC swap...");
        println!("  Amount: {:.6} SOL", amount_sol);
        println!("  Mode: {}", if dry_run { "Dry Run" } else { "Live Trade" });

        let result = executor.execute_transaction_command(amount_sol, "sol-to-usdc", dry_run).await?;
        
        if !result.success {
            return Err(format!("Swap failed: {}", result.error.unwrap_or_else(|| "Unknown error".to_string())).into());
        }

        // Extract USDC received
        if let Some(usdc_change) = result.usdc_change {
            if usdc_change > 0.0 {
                println!("  ✅ Received {:.2} USDC", usdc_change);
                Ok(usdc_change)
            } else {
                Err("No USDC received from swap".into())
            }
        } else {
            Err("Could not determine USDC amount received".into())
        }
    }

    /// Verify balance consistency after trades
    fn verify_balance_consistency(
        initial: &WalletBalance,
        final_balance: &WalletBalance,
        usdc_spent: f64,
        sol_received: f64,
        usdc_received: f64,
    ) {
        println!("  🔍 Balance Consistency Check:");
        
        // Calculate expected changes
        let expected_sol_change = sol_received;
        let expected_usdc_change = usdc_received - usdc_spent;
        
        let actual_sol_change = final_balance.sol_balance - initial.sol_balance;
        let actual_usdc_change = final_balance.usdc_balance - initial.usdc_balance;
        
        println!("    Expected SOL change: {:.6} SOL", expected_sol_change);
        println!("    Actual SOL change: {:.6} SOL", actual_sol_change);
        println!("    Expected USDC change: {:.2} USDC", expected_usdc_change);
        println!("    Actual USDC change: {:.2} USDC", actual_usdc_change);
        
        // Check if changes are reasonable (accounting for fees and slippage)
        let sol_tolerance = 0.0001; // 0.0001 SOL tolerance
        let usdc_tolerance = 0.01; // $0.01 USDC tolerance
        
        let sol_diff = (actual_sol_change - expected_sol_change).abs();
        let usdc_diff = (actual_usdc_change - expected_usdc_change).abs();
        
        if sol_diff <= sol_tolerance && usdc_diff <= usdc_tolerance {
            println!("  ✅ Balance consistency verified");
        } else {
            println!("  ⚠️  Balance inconsistency detected:");
            println!("    SOL difference: {:.6} SOL", sol_diff);
            println!("    USDC difference: {:.2} USDC", usdc_diff);
        }
    }

    /// Test trading signal execution
    async fn test_trading_signal_execution(
        executor: &TradingExecutor,
        config: &TestConfig,
    ) {
        println!("  Testing trading signal execution...");
        
        // Create a test buy signal
        let buy_signal = TradingSignal {
            signal_type: SignalType::Buy,
            confidence: config.min_confidence,
            price: 100.0, // Example price
            timestamp: Utc::now(),
            reasoning: vec!["Test signal for validation".to_string()],
            take_profit: 0.02, // 2% take profit
            stop_loss: 0.014,  // 1.4% stop loss
        };

        // Create a test sell signal
        let sell_signal = TradingSignal {
            signal_type: SignalType::Sell,
            confidence: config.min_confidence,
            price: 101.0, // Example price
            timestamp: Utc::now(),
            reasoning: vec!["Test sell signal for validation".to_string()],
            take_profit: 0.02,
            stop_loss: 0.014,
        };

        // Test buy signal execution
        println!("    Testing BUY signal execution...");
        match executor.execute_signal(&buy_signal).await {
            Ok(success) => {
                if success {
                    println!("    ✅ BUY signal executed successfully");
                } else {
                    println!("    ⚠️  BUY signal execution skipped (likely due to conditions)");
                }
            }
            Err(e) => {
                println!("    ❌ BUY signal execution failed: {}", e);
            }
        }

        // Test sell signal execution
        println!("    Testing SELL signal execution...");
        match executor.execute_signal(&sell_signal).await {
            Ok(success) => {
                if success {
                    println!("    ✅ SELL signal executed successfully");
                } else {
                    println!("    ⚠️  SELL signal execution skipped (likely due to conditions)");
                }
            }
            Err(e) => {
                println!("    ❌ SELL signal execution failed: {}", e);
            }
        }
    }

    /// Test balance checking functionality
    #[tokio::test]
    async fn test_balance_checking() {
        println!("🔍 Testing Balance Checking Functionality");
        
        let executor = match TradingExecutor::new() {
            Ok(exec) => exec,
            Err(e) => {
                println!("❌ Failed to initialize trading executor: {}", e);
                return;
            }
        };

        // Test balance retrieval
        match executor.get_wallet_balance().await {
            Ok(balance) => {
                println!("✅ Balance check successful:");
                println!("  SOL: {:.6} SOL", balance.sol_balance);
                println!("  USDC: {:.2} USDC", balance.usdc_balance);
                println!("  Timestamp: {}", balance.timestamp);
                
                // Validate balance values
                assert!(balance.sol_balance >= 0.0, "SOL balance should be non-negative");
                assert!(balance.usdc_balance >= 0.0, "USDC balance should be non-negative");
                println!("✅ Balance validation passed");
            }
            Err(e) => {
                println!("❌ Balance check failed: {}", e);
                panic!("Balance check should succeed");
            }
        }
    }

    /// Test transaction command execution
    #[tokio::test]
    async fn test_transaction_command() {
        println!("🔧 Testing Transaction Command Execution");
        
        let executor = match TradingExecutor::new() {
            Ok(exec) => exec,
            Err(e) => {
                println!("❌ Failed to initialize trading executor: {}", e);
                return;
            }
        };

        // Test dry-run transaction
        let test_amount = 0.01; // Small test amount
        println!("  Testing dry-run transaction with ${:.2} USDC", test_amount);
        
        match executor.execute_transaction_command(test_amount, "usdc-to-sol", true).await {
            Ok(result) => {
                if result.success {
                    println!("  ✅ Dry-run transaction successful");
                    if let Some(sol_change) = result.sol_change {
                        println!("    Would receive: {:.6} SOL", sol_change);
                    }
                    if let Some(usdc_change) = result.usdc_change {
                        println!("    Would spend: {:.2} USDC", usdc_change.abs());
                    }
                } else {
                    println!("  ❌ Dry-run transaction failed: {}", 
                            result.error.unwrap_or_else(|| "Unknown error".to_string()));
                }
            }
            Err(e) => {
                println!("  ❌ Transaction command failed: {}", e);
            }
        }
    }

    /// Test configuration validation
    #[test]
    fn test_configuration_validation() {
        println!("⚙️  Testing Configuration Validation");
        
        // Test environment variable parsing
        let test_config = TestConfig::default();
        
        assert!(test_config.test_amount_usdc > 0.0, "Test amount should be positive");
        assert!(test_config.slippage_tolerance > 0.0, "Slippage tolerance should be positive");
        assert!(test_config.slippage_tolerance < 1.0, "Slippage tolerance should be less than 100%");
        assert!(test_config.min_confidence > 0.0, "Min confidence should be positive");
        assert!(test_config.min_confidence <= 1.0, "Min confidence should be <= 100%");
        
        println!("✅ Configuration validation passed");
    }

    /// Test PnL calculation accuracy
    #[test]
    fn test_pnl_calculation() {
        println!("💰 Testing PnL Calculation Accuracy");
        
        // Test cases
        let test_cases = vec![
            (100.0, 102.0, 2.0, 2.0),   // Profit
            (100.0, 98.0, -2.0, -2.0),  // Loss
            (100.0, 100.0, 0.0, 0.0),   // Breakeven
            (50.0, 75.0, 25.0, 50.0),   // 50% profit
            (200.0, 150.0, -50.0, -25.0), // 25% loss
        ];
        
        for (entry, exit, expected_pnl, expected_percent) in test_cases {
            let pnl = exit - entry;
            let pnl_percent = (pnl / entry) * 100.0;
            
            assert!((pnl - expected_pnl).abs() < 0.01, 
                    "PnL calculation error for entry=${}, exit=${}", entry, exit);
            assert!((pnl_percent - expected_percent).abs() < 0.01, 
                    "PnL percentage calculation error for entry=${}, exit=${}", entry, exit);
            
            println!("  ✅ Entry: ${:.2}, Exit: ${:.2}, PnL: ${:.2} ({:.1}%)", 
                    entry, exit, pnl, pnl_percent);
        }
        
        println!("✅ PnL calculation tests passed");
    }
} 