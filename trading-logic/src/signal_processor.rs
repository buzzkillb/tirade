use crate::models::{TradingSignal, SignalType, PriceFeed};
use crate::position_manager::{Position, PositionType, PositionManager};
use crate::trading_executor::TradingExecutor;
use crate::database_service::DatabaseService;
use crate::ml_strategy::{MLStrategy, TradeResult};
use anyhow::Result;
use chrono::Utc;
use tracing::{info, warn, error};

#[derive(Debug, Clone)]
pub enum OverrideLevel {
    None,           // Full neural control
    Loose,          // Wide safety net
    Moderate,       // Medium safety net  
    Strict,         // Tight safety net
}

#[derive(Debug, Clone)]
pub struct DynamicConfidenceConfig {
    pub high_confidence_threshold: f64,    // 70%+ confidence
    pub moderate_confidence_threshold: f64, // 40-70% confidence
    pub high_accuracy_threshold: f64,      // 60%+ recent accuracy
    pub moderate_accuracy_threshold: f64,  // 40-60% recent accuracy
    pub min_predictions_for_accuracy: u64, // Minimum predictions before trusting accuracy
}

impl Default for DynamicConfidenceConfig {
    fn default() -> Self {
        Self {
            high_confidence_threshold: std::env::var("DYNAMIC_HIGH_CONFIDENCE_THRESHOLD")
                .unwrap_or_else(|_| "0.70".to_string())
                .parse::<f64>()
                .unwrap_or(0.70),
            moderate_confidence_threshold: std::env::var("DYNAMIC_MODERATE_CONFIDENCE_THRESHOLD")
                .unwrap_or_else(|_| "0.40".to_string())
                .parse::<f64>()
                .unwrap_or(0.40),
            high_accuracy_threshold: std::env::var("DYNAMIC_HIGH_ACCURACY_THRESHOLD")
                .unwrap_or_else(|_| "0.60".to_string())
                .parse::<f64>()
                .unwrap_or(0.60),
            moderate_accuracy_threshold: std::env::var("DYNAMIC_MODERATE_ACCURACY_THRESHOLD")
                .unwrap_or_else(|_| "0.40".to_string())
                .parse::<f64>()
                .unwrap_or(0.40),
            min_predictions_for_accuracy: std::env::var("DYNAMIC_MIN_PREDICTIONS")
                .unwrap_or_else(|_| "20".to_string())
                .parse::<u64>()
                .unwrap_or(20),
        }
    }
}

pub struct SignalProcessor {
    trading_pair: String,
    last_buy_wallet_index: Option<usize>, // Track rotation for buy signals
    dynamic_config: DynamicConfidenceConfig, // Dynamic confidence configuration
    last_trade_time: Option<chrono::DateTime<Utc>>, // Track last trade time for cooldown
    last_sell_price: Option<f64>, // Track last sell price for retracement filter
    last_sell_time: Option<chrono::DateTime<Utc>>, // Track last sell time for retracement filter
}

impl SignalProcessor {
    pub fn new(trading_pair: String) -> Self {
        Self { 
            trading_pair,
            last_buy_wallet_index: None,
            dynamic_config: DynamicConfidenceConfig::default(),
            last_trade_time: None,
            last_sell_price: None,
            last_sell_time: None,
        }
    }

    /// Restore last sell information from ML trade history on startup
    pub async fn restore_last_sell_from_database(&mut self, database: &crate::database_service::DatabaseService, _wallet_address: &str) -> Result<()> {
        // Use ML trade history to get the most recent sell (exit) price
        let database_url = std::env::var("DATABASE_URL").unwrap_or_else(|_| "http://localhost:8080".to_string());
        let client = reqwest::Client::new();
        
        // Query ML trades for the most recent trade (limit=1)
        let url = format!("{}/ml/trades/SOL%2FUSDC?limit=1", database_url);
        
        match client.get(&url).send().await {
            Ok(response) => {
                if response.status().is_success() {
                    match response.json::<serde_json::Value>().await {
                        Ok(response_json) => {
                            // Handle both array and object responses
                            let trades = match response_json {
                                serde_json::Value::Array(trades_array) => trades_array,
                                serde_json::Value::Object(obj) => {
                                    if let Some(serde_json::Value::Array(trades_array)) = obj.get("data") {
                                        trades_array.clone()
                                    } else {
                                        Vec::new()
                                    }
                                },
                                _ => Vec::new(),
                            };
                            
                            if let Some(last_trade) = trades.first() {
                                if let (Some(exit_price), Some(exit_time_str)) = (
                                    last_trade.get("exit_price").and_then(|v| v.as_f64()),
                                    last_trade.get("exit_time").and_then(|v| v.as_str())
                                ) {
                                    // Parse the exit time
                                    if let Ok(exit_time) = chrono::DateTime::parse_from_rfc3339(exit_time_str) {
                                        self.last_sell_price = Some(exit_price);
                                        self.last_sell_time = Some(exit_time.with_timezone(&chrono::Utc));
                                        info!("🔄 RETRACEMENT RESTORED: Last sell ${:.4} at {} from ML trade history", 
                                              exit_price, exit_time.format("%H:%M:%S"));
                                    } else {
                                        warn!("⚠️ RETRACEMENT RESTORE: Could not parse exit time from ML trade");
                                    }
                                } else {
                                    info!("🔄 RETRACEMENT RESTORED: ML trade found but missing exit price/time");
                                }
                            } else {
                                info!("🔄 RETRACEMENT RESTORED: No ML trades found in database");
                            }
                        }
                        Err(e) => {
                            warn!("⚠️ RETRACEMENT RESTORE FAILED: Could not parse ML trade response: {}", e);
                        }
                    }
                } else {
                    warn!("⚠️ RETRACEMENT RESTORE FAILED: ML trade query failed: HTTP {}", response.status());
                }
            }
            Err(e) => {
                warn!("⚠️ RETRACEMENT RESTORE FAILED: Could not query ML trades: {}", e);
            }
        }
        
        Ok(())
    }

    /// Check if enough time has passed since last trade (cooldown mechanism)
    fn check_trade_cooldown(&self) -> bool {
        let cooldown_seconds = std::env::var("TRADE_COOLDOWN_SECONDS")
            .unwrap_or_else(|_| "300".to_string())  // 5 minutes default
            .parse::<i64>()
            .unwrap_or(300);
        
        if let Some(last_trade) = self.last_trade_time {
            let time_since_last_trade = (Utc::now() - last_trade).num_seconds();
            if time_since_last_trade < cooldown_seconds {
                info!("⏰ Trade cooldown active: {}s remaining ({}s required)", 
                      cooldown_seconds - time_since_last_trade, cooldown_seconds);
                return false;
            }
        }
        true
    }

    /// Update the last trade time
    fn update_last_trade_time(&mut self) {
        self.last_trade_time = Some(Utc::now());
    }

    /// Determine override level based on neural network confidence and performance
    fn determine_override_level(&self, _ml_strategy: &MLStrategy) -> OverrideLevel {
        // Overrides completely disabled - full neural network control
        OverrideLevel::None
    }

    /// Get override thresholds based on override level
    fn get_override_thresholds(&self, override_level: &OverrideLevel) -> Option<(f64, f64)> {
        match override_level {
            OverrideLevel::None => {
                None // No overrides - full neural control
            },
            OverrideLevel::Loose => {
                info!("🛡️ Safety Net: LOOSE overrides - Stop: -15%, Take Profit: +25%");
                Some((-0.15, 0.25)) // -15% stop loss, +25% take profit
            },
            OverrideLevel::Moderate => {
                info!("🛡️ Safety Net: MODERATE overrides - Stop: -10%, Take Profit: +18%");
                Some((-0.10, 0.18)) // -10% stop loss, +18% take profit
            },
            OverrideLevel::Strict => {
                info!("🛡️ Safety Net: STRICT overrides - Stop: -6%, Take Profit: +12%");
                Some((-0.06, 0.12)) // -6% stop loss, +12% take profit
            }
        }
    }

    pub async fn handle_buy_signal(
        &mut self,
        signal: &TradingSignal,
        executors: &mut [TradingExecutor],
        position_manager: &mut PositionManager,
        database: &DatabaseService,
    ) -> Result<()> {
        // Check trade cooldown to prevent rapid-fire trading
        if !self.check_trade_cooldown() {
            info!("🟢 BUY signal received but trade cooldown active - skipping to prevent micro-trading");
            return Ok(());
        }
        
        // TIME + OPPORTUNITY COST RETRACEMENT FILTER
        if let (Some(last_sell_price), Some(last_sell_time)) = (self.last_sell_price, self.last_sell_time) {
            let current_price = signal.price;
            let time_since_sell = (Utc::now() - last_sell_time).num_minutes();
            let price_move_since_sell = (current_price - last_sell_price) / last_sell_price;
            
            // Retracement requirements - RELAXED for faster re-entry
            let retracement_requirement = if time_since_sell <= 60 { // 1 hour
                0.005 // Need 0.5% below last sell price
            } else if time_since_sell <= 180 { // 3 hours
                0.002 // Need 0.2% below last sell price
            } else {
                0.0 // No requirement after 3 hours
            };
            
            let opportunity_cost_threshold = 0.008; // 0.8% move up = override (much lower)
            
            // Check if we should block the buy
            let price_too_high = current_price > (last_sell_price - (last_sell_price * retracement_requirement));
            let opportunity_cost_triggered = price_move_since_sell > opportunity_cost_threshold;
            let time_override = time_since_sell > 180; // 3 hours (reduced from 6)
            
            // NEW: Strong uptrend override - if price is moving up strongly, allow re-entry
            let strong_uptrend_override = price_move_since_sell > 0.005 && time_since_sell >= 30; // 0.5% up after 30min
            
            if price_too_high && !opportunity_cost_triggered && !time_override && !strong_uptrend_override {
                info!("🚫 BUY BLOCKED: ${:.4} too high vs sell ${:.4} ({}min ago, need ${:.4})", 
                      current_price, last_sell_price, time_since_sell, last_sell_price - (last_sell_price * retracement_requirement));
                return Ok(());
            } else if opportunity_cost_triggered {
                info!("✅ BUY ALLOWED: Opportunity cost override (+{:.2}% move)", price_move_since_sell * 100.0);
            } else if strong_uptrend_override {
                info!("✅ BUY ALLOWED: Strong uptrend override (+{:.2}% move after {}min)", price_move_since_sell * 100.0, time_since_sell);
            } else if time_override {
                info!("✅ BUY ALLOWED: Time override ({}min)", time_since_sell);
            } else {
                info!("✅ BUY ALLOWED: Price retracement OK (${:.4} vs ${:.4})", current_price, last_sell_price);
            }
        } else {
            info!("🔄 RETRACEMENT FILTER: No previous sell data - allowing buy");
        }
        
        info!("🟢 Processing BUY signal with staggered rotation across {} wallets", executors.len());
        
        // Single position per wallet: Each wallet can have max 1 position
        // Don't prevent other wallets from trading if one wallet has a position
        
        // STAGGERED STRATEGY: Use round-robin rotation to select next wallet
        let next_wallet_index = self.get_next_buy_wallet(executors.len(), position_manager);
        
        if let Some(wallet_index) = next_wallet_index {
            let executor = &executors[wallet_index];
            
            if !position_manager.has_position(wallet_index) {
                info!("💰 {} executing BUY signal (rotation: wallet {})", 
                      executor.get_wallet_name(), wallet_index + 1);
                
                match executor.execute_signal(signal, None).await {
                    Ok((success, quantity, execution_price, usdc_change)) => {
                        if success {
                            // Use actual execution price from Jupiter, fallback to signal price
                            let entry_price = execution_price.unwrap_or(signal.price);
                            
                            info!("💰 {} using execution price: ${:.4} (Jupiter: {:?}, Signal: ${:.4})", 
                                  executor.get_wallet_name(), entry_price, execution_price, signal.price);
                            
                            // Use USDC change from transaction result (negative = spent)
                            let usdc_spent = usdc_change.filter(|&change| change < 0.0);
                            
                            // Create position record
                            let position = Position {
                                position_id: None, // Will be set after database post
                                entry_price,
                                entry_time: signal.timestamp,
                                quantity: quantity.unwrap_or(1.0),
                                position_type: PositionType::Long,
                                usdc_spent,
                            };

                            // Post to database with actual USDC spent from transaction
                            let wallet_address = executor.get_wallet_address()?;
                            
                            if let Some(usdc_amount) = usdc_spent {
                                info!("💰 USDC-based PnL: Recording ${:.2} USDC spent for position entry", usdc_amount.abs());
                            }
                            
                            match database.create_position_with_usdc(
                                &wallet_address,
                                &self.trading_pair,
                                "long",
                                position.entry_price,
                                position.quantity,
                                usdc_spent
                            ).await {
                                Ok(position_id) => {
                                    let mut updated_position = position;
                                    updated_position.position_id = Some(position_id);
                                    position_manager.set_position(wallet_index, Some(updated_position));
                                    
                                    // Update rotation tracker
                                    self.last_buy_wallet_index = Some(wallet_index);
                                    
                                    // Update trade cooldown timer
                                    self.update_last_trade_time();
                                    
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
                            Ok((success, quantity, execution_price, usdc_change)) => {
                                if success {
                                    // Use actual execution price from Jupiter, fallback to signal price
                                    let entry_price = execution_price.unwrap_or(signal.price);
                                    
                                    info!("💰 {} using execution price: ${:.4} (Jupiter: {:?}, Signal: ${:.4})", 
                                          fallback_executor.get_wallet_name(), entry_price, execution_price, signal.price);
                                    
                                    // Use USDC change from transaction result (negative = spent)
                                    let usdc_spent = usdc_change.filter(|&change| change < 0.0);
                                    
                                    let position = Position {
                                        position_id: None,
                                        entry_price,
                                        entry_time: signal.timestamp,
                                        quantity: quantity.unwrap_or(1.0),
                                        position_type: PositionType::Long,
                                        usdc_spent,
                                    };

                                    let wallet_address = fallback_executor.get_wallet_address()?;
                                    
                                    if let Some(usdc_amount) = usdc_spent {
                                        info!("💰 USDC-based PnL: Recording ${:.2} USDC spent for fallback position entry", usdc_amount.abs());
                                    }
                                    
                                    match database.create_position_with_usdc(
                                        &wallet_address,
                                        &self.trading_pair,
                                        "long",
                                        position.entry_price,
                                        position.quantity,
                                        usdc_spent
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
        &mut self,
        signal: &TradingSignal,
        executors: &mut [TradingExecutor],
        position_manager: &mut PositionManager,
        database: &DatabaseService,
        ml_strategy: &mut MLStrategy,
    ) -> Result<()> {
        // Check trade cooldown to prevent rapid-fire trading
        if !self.check_trade_cooldown() {
            info!("🔴 SELL signal received but trade cooldown active - skipping to prevent micro-trading");
            return Ok(());
        }
        
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
                Ok((success, _, execution_price, usdc_change)) => {
                    if success {
                        // Use actual execution price from Jupiter, fallback to signal price
                        let exit_price = execution_price.unwrap_or(signal.price);
                        
                        info!("💱 {} using exit price: ${:.4} (Jupiter: {:?}, Signal: ${:.4})", 
                              executor.get_wallet_name(), exit_price, execution_price, signal.price);
                        
                        // Calculate PnL using actual execution prices (fallback method)
                        let price_based_pnl = (exit_price - position.entry_price) / position.entry_price;
                        
                        // Use USDC change from transaction result (positive = received)
                        let usdc_received = usdc_change.filter(|&change| change > 0.0);
                        
                        if let Some(usdc_amount) = usdc_received {
                            info!("💰 USDC-based PnL: Received ${:.2} USDC for position exit", usdc_amount);
                        }
                        
                        // Database operations - use actual exit price and USDC received
                        if let Some(position_id) = &position.position_id {
                            match database.close_position_with_usdc(position_id, exit_price, usdc_received).await {
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

                        // Calculate actual USDC-based PnL if available
                        let (actual_pnl, success) = if let (Some(usdc_received), Some(usdc_spent)) = (usdc_received, position.usdc_spent) {
                            // Use actual USDC flow for PnL calculation
                            let usdc_pnl_percentage = (usdc_received - usdc_spent.abs()) / usdc_spent.abs();
                            let is_profitable = usdc_received > usdc_spent.abs();
                            info!("💰 Neural Learning: Using actual USDC PnL: {:.2}% (${:.2} → ${:.2})", 
                                  usdc_pnl_percentage * 100.0, usdc_spent.abs(), usdc_received);
                            (usdc_pnl_percentage, is_profitable)
                        } else {
                            // Fallback to price-based PnL
                            warn!("⚠️ Neural Learning: Using price-based PnL fallback (USDC data unavailable)");
                            (price_based_pnl, price_based_pnl > 0.0)
                        };

                        // Record trade result for ML with actual USDC-based PnL
                        let usdc_pnl_amount = if let (Some(received), Some(spent)) = (usdc_received, position.usdc_spent) {
                            Some(received - spent.abs())
                        } else {
                            None
                        };
                        
                        let trade_result = TradeResult {
                            entry_price: position.entry_price,
                            exit_price,
                            pnl: actual_pnl,
                            duration_seconds: (signal.timestamp - position.entry_time).num_seconds(),
                            entry_time: position.entry_time,
                            exit_time: signal.timestamp,
                            success,
                            usdc_spent: position.usdc_spent,
                            usdc_received,
                            usdc_pnl: usdc_pnl_amount,
                        };
                        ml_strategy.record_trade(trade_result).await;

                        // Clear position from memory
                        position_manager.clear_position(wallet_index);
                        
                        // Update trade cooldown timer
                        self.update_last_trade_time();
                        
                        let pnl_emoji = if price_based_pnl > 0.0 { "💰" } else { "💸" };
                        info!("✅ {} closed BEST PERFORMING position: {} PnL: {:.2}% (staggered exit)", 
                              executor.get_wallet_name(), pnl_emoji, price_based_pnl * 100.0);
                        
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
        &mut self,
        prices: &[PriceFeed],
        indicators: &crate::models::TradingIndicators,
        executors: &mut [TradingExecutor],
        position_manager: &mut PositionManager,
        database: &DatabaseService,
        ml_strategy: &mut MLStrategy,
        strategy: &crate::strategy::TradingStrategy,
    ) -> Result<()> {
        // 🧠 DYNAMIC CONFIDENCE SYSTEM: Determine override level based on neural performance
        let override_level = self.determine_override_level(ml_strategy);
        let override_thresholds = self.get_override_thresholds(&override_level);
        let current_price = prices.last().map(|p| p.price).unwrap_or(0.0);
        
        // Debug price data ordering
        if prices.len() >= 2 {
            let first_price = prices.first().map(|p| p.price).unwrap_or(0.0);
            let last_price = prices.last().map(|p| p.price).unwrap_or(0.0);
            let first_time = prices.first().map(|p| p.timestamp).unwrap();
            let last_time = prices.last().map(|p| p.timestamp).unwrap();
            
            info!("🔍 PRICE DATA DEBUG: First=${:.4} ({}), Last=${:.4} ({}), Count={}", 
                  first_price, first_time.format("%H:%M:%S"), 
                  last_price, last_time.format("%H:%M:%S"), 
                  prices.len());
        }
        
        info!("🔍 EXIT CONDITIONS CHECK: Current price ${:.4}", current_price);
        
        // Check each position for exit conditions
        for (wallet_index, executor) in executors.iter().enumerate() {
            if let Some(position) = position_manager.get_position(wallet_index).cloned() {
                let pnl = (current_price - position.entry_price) / position.entry_price;
                let rsi = indicators.rsi_fast.unwrap_or(50.0);
                let momentum_decay = strategy.detect_momentum_decay(prices);
                
                info!("📊 {} POSITION CHECK: Entry ${:.4} → Current ${:.4} = {:.2}% PnL", 
                      executor.get_wallet_name(), position.entry_price, current_price, pnl * 100.0);
                
                // 🧠 DYNAMIC CONFIDENCE SYSTEM: Apply exit conditions based on override level
                let should_exit = match override_thresholds {
                    None => {
                        // NO OVERRIDES: Full neural control with stop-loss and take profit
                        let position_age_minutes = (Utc::now() - position.entry_time).num_minutes();
                        
                        // Use environment variables for exit thresholds
                        let min_exit_hold_time = std::env::var("MIN_EXIT_HOLD_TIME_MINUTES")
                            .unwrap_or_else(|_| "5".to_string())
                            .parse::<i64>()
                            .unwrap_or(5);
                        
                        let min_exit_profit = std::env::var("MIN_EXIT_PROFIT_PERCENT")
                            .unwrap_or_else(|_| "2.0".to_string())
                            .parse::<f64>()
                            .unwrap_or(2.0) / 100.0; // Convert percentage to decimal
                        
                        let rsi_overbought_threshold = std::env::var("RSI_OVERBOUGHT_THRESHOLD")
                            .unwrap_or_else(|_| "85.0".to_string())
                            .parse::<f64>()
                            .unwrap_or(85.0);
                        
                        // NEW: Stop-loss and take profit conditions
                        let stop_loss_threshold = -0.02; // -2% stop loss
                        let take_profit_threshold = 0.04; // +4% take profit
                        
                        // Exit conditions
                        let stop_loss_condition = pnl < stop_loss_threshold;
                        let take_profit_condition = pnl > take_profit_threshold && position_age_minutes >= min_exit_hold_time;
                        let rsi_overbought_condition = rsi > rsi_overbought_threshold && pnl > min_exit_profit && position_age_minutes >= min_exit_hold_time;
                        let momentum_decay_condition = momentum_decay && pnl > (min_exit_profit * 1.5) && position_age_minutes >= min_exit_hold_time;
                        
                        // Log only if conditions are close to triggering or actually triggered
                        if stop_loss_condition {
                            info!("🚨 {} STOP LOSS TRIGGERED: {:.2}% loss", executor.get_wallet_name(), pnl * 100.0);
                        } else if take_profit_condition {
                            info!("💰 {} TAKE PROFIT TRIGGERED: {:.2}% gain", executor.get_wallet_name(), pnl * 100.0);
                        } else if rsi_overbought_condition {
                            info!("📈 {} RSI EXIT TRIGGERED: RSI {:.1} overbought, {:.2}% gain", executor.get_wallet_name(), rsi, pnl * 100.0);
                        } else if momentum_decay_condition {
                            info!("📉 {} MOMENTUM EXIT TRIGGERED: Decay detected, {:.2}% gain", executor.get_wallet_name(), pnl * 100.0);
                        }
                        
                        stop_loss_condition || take_profit_condition || rsi_overbought_condition || momentum_decay_condition
                    },
                    Some((stop_loss_threshold, take_profit_threshold)) => {
                        // OVERRIDES ACTIVE: Apply dynamic thresholds
                        let position_age_minutes = (Utc::now() - position.entry_time).num_minutes();
                        
                        // Use environment variables for exit thresholds
                        let min_exit_hold_time = std::env::var("MIN_EXIT_HOLD_TIME_MINUTES")
                            .unwrap_or_else(|_| "3".to_string())
                            .parse::<i64>()
                            .unwrap_or(3);
                        
                        let min_exit_profit = std::env::var("MIN_EXIT_PROFIT_PERCENT")
                            .unwrap_or_else(|_| "2.5".to_string())
                            .parse::<f64>()
                            .unwrap_or(2.5) / 100.0; // Convert percentage to decimal
                        
                        let rsi_overbought_threshold = std::env::var("RSI_OVERBOUGHT_THRESHOLD")
                            .unwrap_or_else(|_| "80.0".to_string())
                            .parse::<f64>()
                            .unwrap_or(80.0);
                        
                        let rsi_overbought_condition = rsi > rsi_overbought_threshold && pnl > min_exit_profit && position_age_minutes >= min_exit_hold_time;
                        let momentum_decay_condition = momentum_decay && pnl > min_exit_profit && position_age_minutes >= min_exit_hold_time;
                        let take_profit_condition = pnl > take_profit_threshold && position_age_minutes >= min_exit_hold_time;
                        let stop_loss_condition = pnl < stop_loss_threshold;
                        
                        info!("🛡️ {} OVERRIDE ANALYSIS:", executor.get_wallet_name());
                        info!("   RSI Overbought: RSI={:.1} > {:.1} && PnL={:.2}% > {:.1}% && Age={}min >= {}min = {}", 
                              rsi, rsi_overbought_threshold, pnl * 100.0, min_exit_profit * 100.0, position_age_minutes, min_exit_hold_time, rsi_overbought_condition);
                        info!("   Momentum Decay: Decay={} && PnL={:.2}% > {:.1}% && Age={}min >= {}min = {}", 
                              momentum_decay, pnl * 100.0, min_exit_profit * 100.0, position_age_minutes, min_exit_hold_time, momentum_decay_condition);
                        info!("   Take Profit: PnL={:.2}% > {:.1}% && Age={}min >= {}min = {}", 
                              pnl * 100.0, take_profit_threshold * 100.0, position_age_minutes, min_exit_hold_time, take_profit_condition);
                        info!("   Stop Loss: PnL={:.2}% < {:.1}% = {}", 
                              pnl * 100.0, stop_loss_threshold * 100.0, stop_loss_condition);
                        
                        rsi_overbought_condition || momentum_decay_condition || take_profit_condition || stop_loss_condition
                    }
                };
                
                if should_exit {
                    let exit_reason = match override_thresholds {
                        None => {
                            // Neural control - determine reason
                            let min_exit_profit = std::env::var("MIN_EXIT_PROFIT_PERCENT")
                                .unwrap_or_else(|_| "2.0".to_string())
                                .parse::<f64>()
                                .unwrap_or(2.0);
                            
                            let rsi_overbought_threshold = std::env::var("RSI_OVERBOUGHT_THRESHOLD")
                                .unwrap_or_else(|_| "85.0".to_string())
                                .parse::<f64>()
                                .unwrap_or(85.0);
                            
                            // Check exit conditions in priority order
                            if pnl < -0.02 {
                                "STOP LOSS (-2.0%)".to_string()
                            } else if pnl > 0.04 {
                                "TAKE PROFIT (+4.0%)".to_string()
                            } else if rsi > rsi_overbought_threshold && pnl > (min_exit_profit / 100.0) {
                                format!("NEURAL: RSI OVERBOUGHT (+{:.1}%)", min_exit_profit)
                            } else if momentum_decay && pnl > (min_exit_profit * 1.5 / 100.0) {
                                format!("NEURAL: MOMENTUM DECAY (+{:.1}%)", min_exit_profit * 1.5)
                            } else {
                                "NEURAL: TECHNICAL EXIT".to_string()
                            }
                        },
                        Some((stop_loss_threshold, take_profit_threshold)) => {
                            // Override control - determine reason
                            let min_exit_profit = std::env::var("MIN_EXIT_PROFIT_PERCENT")
                                .unwrap_or_else(|_| "2.5".to_string())
                                .parse::<f64>()
                                .unwrap_or(2.5);
                            
                            let rsi_overbought_threshold = std::env::var("RSI_OVERBOUGHT_THRESHOLD")
                                .unwrap_or_else(|_| "80.0".to_string())
                                .parse::<f64>()
                                .unwrap_or(80.0);
                            
                            if pnl < stop_loss_threshold {
                                format!("STOP LOSS ({:.1}%)", stop_loss_threshold * 100.0)
                            } else if pnl > take_profit_threshold {
                                format!("TAKE PROFIT (+{:.1}%)", take_profit_threshold * 100.0)
                            } else if rsi > rsi_overbought_threshold && pnl > (min_exit_profit / 100.0) {
                                format!("RSI OVERBOUGHT (+{:.1}%)", min_exit_profit)
                            } else if momentum_decay && pnl > (min_exit_profit / 100.0) {
                                format!("MOMENTUM DECAY (+{:.1}%)", min_exit_profit)
                            } else {
                                "TECHNICAL EXIT".to_string()
                            }
                        }
                    };
                    
                    info!("🚪 {} EXIT CONDITION TRIGGERED: {} | PnL {:.2}%", 
                          executor.get_wallet_name(), exit_reason, pnl * 100.0);
                    
                    // Create sell signal for this position
                    let exit_signal = TradingSignal {
                        signal_type: SignalType::Sell,
                        price: current_price,
                        timestamp: Utc::now(),
                        confidence: 0.8, // High confidence for exit conditions
                        reasoning: vec!["Exit condition triggered".to_string()],
                    };
                    
                    // Execute sell for this specific wallet
                    if let Ok((success, _, execution_price, usdc_change)) = executor.execute_signal(&exit_signal, Some(position.quantity)).await {
                        if success {
                            // Use actual execution price from Jupiter, fallback to current price
                            let exit_price = execution_price.unwrap_or(current_price);
                            
                            // Recalculate PnL with actual exit price
                            let actual_pnl = (exit_price - position.entry_price) / position.entry_price;
                            
                            // Use USDC change from transaction result (positive = received)
                            let usdc_received = usdc_change.filter(|&change| change > 0.0);
                            
                            if let Some(usdc_amount) = usdc_received {
                                info!("💰 USDC-based PnL: Received ${:.2} USDC for exit condition", usdc_amount);
                            }
                            
                            // Handle position closure with USDC received
                            if let Some(position_id) = &position.position_id {
                                if let Err(e) = database.close_position_with_usdc(position_id, exit_price, usdc_received).await {
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
                                usdc_spent: position.usdc_spent,
                                usdc_received: None, // Not available in this context
                                usdc_pnl: None, // Not available in this context
                            };
                            
                            // Record trade for ML and neural learning
                            ml_strategy.record_trade(trade_result).await;
                            
                            // Clear position
                            position_manager.clear_position(wallet_index);
                            
                            // Track last sell for retracement filter
                            self.last_sell_price = Some(exit_price);
                            self.last_sell_time = Some(Utc::now());
                            info!("🔄 RETRACEMENT TRACKER: Recorded sell at ${:.4} for future buy filtering", exit_price);
                            
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
    fn find_best_performing_wallet(&self, signal_price: f64, executors: &[TradingExecutor], position_manager: &PositionManager) -> Option<usize> {
        let mut best_wallet: Option<usize> = None;
        let mut best_pnl = f64::NEG_INFINITY;

        info!("🔍 SELL SIGNAL PnL CALCULATION: Using signal price ${:.4}", signal_price);

        for (wallet_index, executor) in executors.iter().enumerate() {
            if let Some(position) = position_manager.get_position(wallet_index) {
                let pnl = (signal_price - position.entry_price) / position.entry_price;
                
                info!("📊 {} SELL PnL: Entry ${:.4} → Signal ${:.4} = {:.2}%", 
                      executor.get_wallet_name(), position.entry_price, signal_price, pnl * 100.0);
                
                if pnl > best_pnl {
                    best_pnl = pnl;
                    best_wallet = Some(wallet_index);
                }
            }
        }

        if let Some(wallet_index) = best_wallet {
            let executor = &executors[wallet_index];
            info!("🏆 Best performer: {} with {:.2}% PnL (using signal price)", 
                  executor.get_wallet_name(), best_pnl * 100.0);
        }

        best_wallet
    }
}