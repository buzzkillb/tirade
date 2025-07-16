use crate::models::{TradingSignal, SignalType, PriceFeed};
use crate::position_manager::{Position, PositionType, PositionManager};
use crate::trading_executor::TradingExecutor;
use crate::database_service::DatabaseService;
use crate::ml_strategy::{MLStrategy, TradeResult};
use anyhow::Result;
use chrono::Utc;
use tracing::{info, warn, error};

pub struct SignalProcessor {
    trading_pair: String,
    last_buy_wallet_index: Option<usize>, // Track rotation for buy signals
}

impl SignalProcessor {
    pub fn new(trading_pair: String) -> Self {
        Self { 
            trading_pair,
            last_buy_wallet_index: None,
        }
    }

    pub async fn handle_buy_signal(
        &mut self,
        signal: &TradingSignal,
        executors: &[TradingExecutor],
        position_manager: &mut PositionManager,
        database: &DatabaseService,
    ) -> Result<()> {
        info!("🟢 Processing BUY signal with staggered rotation across {} wallets", executors.len());
        
        // Multi-wallet mode: Check if ANY wallet has an active position
        if executors.len() > 1 {
            let active_positions = position_manager.get_active_position_count();
            if active_positions > 0 {
                info!("🚫 Multi-wallet mode: {} active position(s) detected, skipping new BUY signal to prevent over-exposure", active_positions);
                return Ok(());
            }
        }
        
        // STAGGERED STRATEGY: Use round-robin rotation to select next wallet
        let next_wallet_index = self.get_next_buy_wallet(executors.len(), position_manager);
        
        if let Some(wallet_index) = next_wallet_index {
            let executor = &executors[wallet_index];
            
            if !position_manager.has_position(wallet_index) {
                info!("💰 {} executing BUY signal (rotation: wallet {})", 
                      executor.get_wallet_name(), wallet_index + 1);
                
                match executor.execute_signal(signal, None).await {
                    Ok((success, quantity, execution_price)) => {
                        if success {
                            // Use actual execution price from Jupiter, fallback to signal price
                            let entry_price = execution_price.unwrap_or(signal.price);
                            
                            info!("💰 {} using execution price: ${:.4} (Jupiter: {:?}, Signal: ${:.4})", 
                                  executor.get_wallet_name(), entry_price, execution_price, signal.price);
                            
                            // Create position record
                            let position = Position {
                                position_id: None, // Will be set after database post
                                entry_price,
                                entry_time: signal.timestamp,
                                quantity: quantity.unwrap_or(1.0),
                                position_type: PositionType::Long,
                            };

                            // Post to database and get position ID
                            let wallet_address = executor.get_wallet_address()?;
                            match database.create_position(
                                &wallet_address,
                                &self.trading_pair,
                                "long",
                                position.entry_price,
                                position.quantity
                            ).await {
                                Ok(position_id) => {
                                    let mut updated_position = position;
                                    updated_position.position_id = Some(position_id);
                                    position_manager.set_position(wallet_index, Some(updated_position));
                                    
                                    // Update rotation tracker
                                    self.last_buy_wallet_index = Some(wallet_index);
                                    
                                    info!("✅ {} opened position at ${:.4} (staggered entry)", 
                                          executor.get_wallet_name(), entry_price);
                                    return Ok(());
                                }
                                Err(e) => {
                                    error!("❌ Failed to record position for {}: {}", 
                                           executor.get_wallet_name(), e);
                                }
                            }
                        }
                    }
                    Err(e) => {
                        warn!("⚠️ {} failed to execute BUY: {}", executor.get_wallet_name(), e);
                    }
                }
            } else {
                info!("⚠️ {} already has position, looking for next available wallet", 
                      executor.get_wallet_name());
                
                // Try to find any available wallet as fallback
                for (fallback_index, fallback_executor) in executors.iter().enumerate() {
                    if !position_manager.has_position(fallback_index) {
                        info!("💰 {} executing BUY signal (fallback)", fallback_executor.get_wallet_name());
                        
                        match fallback_executor.execute_signal(signal, None).await {
                            Ok((success, quantity, execution_price)) => {
                                if success {
                                    // Use actual execution price from Jupiter, fallback to signal price
                                    let entry_price = execution_price.unwrap_or(signal.price);
                                    
                                    info!("💰 {} using execution price: ${:.4} (Jupiter: {:?}, Signal: ${:.4})", 
                                          fallback_executor.get_wallet_name(), entry_price, execution_price, signal.price);
                                    
                                    let position = Position {
                                        position_id: None,
                                        entry_price,
                                        entry_time: signal.timestamp,
                                        quantity: quantity.unwrap_or(1.0),
                                        position_type: PositionType::Long,
                                    };

                                    let wallet_address = fallback_executor.get_wallet_address()?;
                                    match database.create_position(
                                        &wallet_address,
                                        &self.trading_pair,
                                        "long",
                                        position.entry_price,
                                        position.quantity
                                    ).await {
                                        Ok(position_id) => {
                                            let mut updated_position = position;
                                            updated_position.position_id = Some(position_id);
                                            position_manager.set_position(fallback_index, Some(updated_position));
                                            
                                            self.last_buy_wallet_index = Some(fallback_index);
                                            
                                            info!("✅ {} opened position at ${:.4} (fallback)", 
                                                  fallback_executor.get_wallet_name(), entry_price);
                                            return Ok(());
                                        }
                                        Err(e) => {
                                            error!("❌ Failed to record fallback position: {}", e);
                                        }
                                    }
                                }
                            }
                            Err(e) => {
                                warn!("⚠️ Fallback wallet {} failed: {}", fallback_executor.get_wallet_name(), e);
                            }
                        }
                    }
                }
            }
        }
        
        info!("💤 No available wallets for BUY signal (all wallets have positions)");
        Ok(())
    }

    pub async fn handle_sell_signal(
        &self,
        signal: &TradingSignal,
        executors: &[TradingExecutor],
        position_manager: &mut PositionManager,
        database: &DatabaseService,
        ml_strategy: &mut MLStrategy,
    ) -> Result<()> {
        info!("🔴 Processing SELL signal with profit-based selection across {} wallets", executors.len());
        
        // STAGGERED STRATEGY: Find the best performing wallet to sell first
        let best_wallet_index = self.find_best_performing_wallet(signal.price, executors, position_manager);
        
        if let Some(wallet_index) = best_wallet_index {
            let executor = &executors[wallet_index];
            let position = position_manager.get_position(wallet_index).unwrap(); // Safe because we found it
            
            info!("💱 {} closing BEST PERFORMING position opened at ${:.4}", 
                  executor.get_wallet_name(), position.entry_price);
            
            // Execute the trade
            match executor.execute_signal(signal, Some(position.quantity)).await {
                Ok((success, _, execution_price)) => {
                    if success {
                        // Use actual execution price from Jupiter, fallback to signal price
                        let exit_price = execution_price.unwrap_or(signal.price);
                        
                        info!("💱 {} using exit price: ${:.4} (Jupiter: {:?}, Signal: ${:.4})", 
                              executor.get_wallet_name(), exit_price, execution_price, signal.price);
                        
                        // Calculate PnL using actual execution prices
                        let pnl = (exit_price - position.entry_price) / position.entry_price;
                        
                        // Database operations - use actual exit price
                        if let Some(position_id) = &position.position_id {
                            match database.close_position(position_id, exit_price).await {
                                Ok(_) => {
                                    info!("✅ {} position closed in database with exit price ${:.4}", 
                                          executor.get_wallet_name(), exit_price);
                                }
                                Err(e) => {
                                    error!("❌ Failed to close position in database for {}: {}", 
                                           executor.get_wallet_name(), e);
                                }
                            }
                        }

                        // Record trade result for ML - use actual exit price
                        let trade_result = TradeResult {
                            entry_price: position.entry_price,
                            exit_price,
                            pnl,
                            duration_seconds: (signal.timestamp - position.entry_time).num_seconds(),
                            entry_time: position.entry_time,
                            exit_time: signal.timestamp,
                            success: pnl > 0.0,
                        };
                        ml_strategy.record_trade(trade_result);

                        // Clear position from memory
                        position_manager.clear_position(wallet_index);
                        
                        let pnl_emoji = if pnl > 0.0 { "💰" } else { "💸" };
                        info!("✅ {} closed BEST PERFORMING position: {} PnL: {:.2}% (staggered exit)", 
                              executor.get_wallet_name(), pnl_emoji, pnl * 100.0);
                        
                        // Show remaining positions
                        let remaining_positions = executors.iter().enumerate()
                            .filter(|(i, _)| position_manager.has_position(*i))
                            .count();
                        
                        if remaining_positions > 0 {
                            info!("📊 {} positions still open, waiting for next SELL signal", remaining_positions);
                        }
                    }
                }
                Err(e) => {
                    warn!("⚠️ {} failed to execute SELL: {}", executor.get_wallet_name(), e);
                }
            }
        } else {
            info!("💤 No open positions to close");
        }
        
        Ok(())
    }

    pub async fn check_exit_conditions(
        &self,
        prices: &[PriceFeed],
        indicators: &crate::models::TradingIndicators,
        executors: &[TradingExecutor],
        position_manager: &mut PositionManager,
        database: &DatabaseService,
        ml_strategy: &mut MLStrategy,
        strategy: &crate::strategy::TradingStrategy,
    ) -> Result<()> {
        let current_price = prices.last().map(|p| p.price).unwrap_or(0.0);
        
        // Check each position for exit conditions
        for (wallet_index, executor) in executors.iter().enumerate() {
            if let Some(position) = position_manager.get_position(wallet_index).cloned() {
                let pnl = (current_price - position.entry_price) / position.entry_price;
                
                // Simple exit conditions (can be enhanced)
                let should_exit = 
                    pnl > 0.05 ||  // 5% profit
                    pnl < -0.03 || // 3% loss
                    strategy.detect_momentum_decay(prices) ||
                    (indicators.rsi_fast.unwrap_or(50.0) > 70.0 && pnl > 0.01); // RSI overbought with profit
                
                if should_exit {
                    info!("🚪 {} triggering exit condition: PnL {:.2}%", 
                          executor.get_wallet_name(), pnl * 100.0);
                    
                    // Create sell signal for this position
                    let exit_signal = TradingSignal {
                        signal_type: SignalType::Sell,
                        price: current_price,
                        timestamp: Utc::now(),
                        confidence: 0.8, // High confidence for exit conditions
                        reasoning: vec!["Exit condition triggered".to_string()],
                    };
                    
                    // Execute sell for this specific wallet
                    if let Ok((success, _, execution_price)) = executor.execute_signal(&exit_signal, Some(position.quantity)).await {
                        if success {
                            // Use actual execution price from Jupiter, fallback to current price
                            let exit_price = execution_price.unwrap_or(current_price);
                            
                            // Recalculate PnL with actual exit price
                            let actual_pnl = (exit_price - position.entry_price) / position.entry_price;
                            
                            // Handle position closure
                            if let Some(position_id) = &position.position_id {
                                if let Err(e) = database.close_position(position_id, exit_price).await {
                                    error!("❌ Failed to close position in database: {}", e);
                                }
                            }
                            
                            let trade_result = TradeResult {
                                entry_price: position.entry_price,
                                exit_price,
                                pnl: actual_pnl,
                                duration_seconds: (Utc::now() - position.entry_time).num_seconds(),
                                entry_time: position.entry_time,
                                exit_time: Utc::now(),
                                success: actual_pnl > 0.0,
                            };
                            
                            // Record trade for ML and neural learning
                            ml_strategy.record_trade(trade_result).await;
                            
                            // Clear position
                            position_manager.clear_position(wallet_index);
                            
                            let pnl_emoji = if actual_pnl > 0.0 { "💰" } else { "💸" };
                            info!("✅ {} exit completed: {} PnL: {:.2}% (actual execution price: ${:.4})", 
                                  executor.get_wallet_name(), pnl_emoji, actual_pnl * 100.0, exit_price);
                        }
                    }
                }
            }
        }
        
        Ok(())
    }

    // STAGGERED STRATEGY: Round-robin wallet selection for BUY signals
    fn get_next_buy_wallet(&self, wallet_count: usize, position_manager: &PositionManager) -> Option<usize> {
        if wallet_count == 0 {
            return None;
        }

        // Single wallet mode: Skip round robin and use wallet 0 directly
        if wallet_count == 1 {
            if !position_manager.has_position(0) {
                info!("💰 Single wallet mode: Using wallet 1 for BUY (round robin disabled)");
                return Some(0);
            } else {
                info!("⏳ Single wallet mode: Wallet 1 has open position, skipping BUY");
                return None;
            }
        }

        // Multi-wallet mode: Use round-robin rotation
        let start_index = match self.last_buy_wallet_index {
            Some(last_index) => (last_index + 1) % wallet_count,
            None => 0, // First time, start with wallet 0
        };

        // Try to find an available wallet starting from the rotation point
        for i in 0..wallet_count {
            let wallet_index = (start_index + i) % wallet_count;
            if !position_manager.has_position(wallet_index) {
                info!("🔄 Multi-wallet rotation selected wallet {} for next BUY", wallet_index + 1);
                return Some(wallet_index);
            }
        }

        // No available wallets
        None
    }

    // STAGGERED STRATEGY: Find wallet with highest PnL for SELL signals
    fn find_best_performing_wallet(&self, current_price: f64, executors: &[TradingExecutor], position_manager: &PositionManager) -> Option<usize> {
        let mut best_wallet: Option<usize> = None;
        let mut best_pnl = f64::NEG_INFINITY;

        for (wallet_index, executor) in executors.iter().enumerate() {
            if let Some(position) = position_manager.get_position(wallet_index) {
                let pnl = (current_price - position.entry_price) / position.entry_price;
                
                if pnl > best_pnl {
                    best_pnl = pnl;
                    best_wallet = Some(wallet_index);
                }
                
                info!("📊 {} PnL: {:.2}%", executor.get_wallet_name(), pnl * 100.0);
            }
        }

        if let Some(wallet_index) = best_wallet {
            let executor = &executors[wallet_index];
            info!("🏆 Best performer: {} with {:.2}% PnL", 
                  executor.get_wallet_name(), best_pnl * 100.0);
        }

        best_wallet
    }
}