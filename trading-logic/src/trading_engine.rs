use crate::config::Config;
use crate::models::{PriceFeed, TechnicalIndicators, TradingSignal, SignalType};
use crate::models::{TechnicalIndicator, TradingSignalDb, PositionDb, TradeDb, TradingConfigDb};
use crate::strategy::TradingStrategy;
use crate::trading_executor::TradingExecutor;
use anyhow::{Result, anyhow};
use reqwest::Client;
use std::time::Duration;

use tracing::{info, warn, error, debug};
use chrono::{Utc, DateTime};

pub struct TradingEngine {
    config: Config,
    strategy: TradingStrategy,
    client: Client,
    current_position: Option<Position>,
    last_analysis_time: Option<chrono::DateTime<Utc>>,
    trading_executor: TradingExecutor,
}

#[derive(Debug, Clone)]
struct Position {
    position_id: Option<String>, // Store the database position ID to avoid GET request
    entry_price: f64,
    entry_time: chrono::DateTime<Utc>,
    quantity: f64,
    position_type: PositionType,
}

#[derive(Debug, Clone)]
enum PositionType {
    Long,
    Short,
}

impl TradingEngine {
    pub async fn new(config: Config) -> Result<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(10))
            .build()?;

        let strategy = TradingStrategy::new(config.clone());
        let trading_executor = TradingExecutor::new()?;

        Ok(Self {
            config,
            strategy,
            client,
            current_position: None,
            last_analysis_time: None,
            trading_executor,
        })
    }

    pub async fn run(&mut self) -> Result<()> {
        info!("🚀 Starting Trading Logic Engine...");
        info!("═══════════════════════════════════════════════════════════════");
        info!("  📊 Trading Pair: {}", self.config.trading_pair);
        info!("  🎯 Min Data Points: {}", self.config.min_data_points);
        info!("  ⏱️  Check Interval: {} seconds", self.config.check_interval_secs);
        info!("  🛑 Stop Loss: {:.2}%", self.config.stop_loss_threshold * 100.0);
        info!("  💰 Take Profit: {:.2}%", self.config.take_profit_threshold * 100.0);
        info!("  🌐 Database URL: {}", self.config.database_url);
        info!("  🔄 Trading Execution: {}", if self.trading_executor.is_trading_enabled() { "ENABLED" } else { "PAPER TRADING" });
        info!("  💰 Position Size: {:.1}% of balance", self.trading_executor.get_position_size_percentage() * 100.0);
        info!("  📊 Slippage Tolerance: {:.1}%", self.trading_executor.get_slippage_tolerance() * 100.0);
        info!("  🎯 Min Confidence: {:.1}%", self.trading_executor.get_min_confidence_threshold() * 100.0);
        info!("═══════════════════════════════════════════════════════════════");
        info!("");

        // Post initial trading config to database
        if let Err(e) = self.post_trading_config().await {
            warn!("Failed to post initial trading config: {}", e);
        }

        // Recover positions from database
        info!("🔄 Attempting to recover positions from database...");
        match self.recover_positions().await {
            Ok(_) => {
                if self.current_position.is_some() {
                    info!("✅ Successfully recovered existing position");
                } else {
                    info!("💤 No existing positions found - ready for new trades");
                }
            }
            Err(e) => {
                warn!("⚠️  Failed to recover positions: {}", e);
                info!("🔄 Will retry position recovery on next cycle");
            }
        }

        loop {
            match self.trading_cycle().await {
                Ok(_) => {
                    // Log cycle completion with timestamp
                    let now = Utc::now();
                    if let Some(last_time) = self.last_analysis_time {
                        let duration = now - last_time;
                        debug!("✅ Trading cycle completed in {}ms", duration.num_milliseconds());
                    }
                    self.last_analysis_time = Some(now);
                }
                Err(e) => {
                    error!("❌ Trading cycle failed: {}", e);
                }
            }

            tokio::time::sleep(Duration::from_secs(self.config.check_interval_secs)).await;
        }
    }

    fn create_progress_bar(&self, percentage: f64) -> String {
        let width = 20;
        let filled = (percentage / 100.0 * width as f64) as usize;
        let empty = width - filled;
        
        let filled_char = "█";
        let empty_char = "░";
        
        format!("[{}{}]", 
                filled_char.repeat(filled), 
                empty_char.repeat(empty))
    }

    async fn trading_cycle(&mut self) -> Result<()> {
        // Step 0: ALWAYS attempt position recovery to ensure accurate state
        debug!("🔄 Ensuring accurate position state...");
        match self.recover_positions().await {
            Ok(_) => {
                if self.current_position.is_some() {
                    info!("📈 Position state confirmed: Active position exists");
                } else {
                    info!("💤 Position state confirmed: No active position");
                }
            }
            Err(e) => {
                warn!("⚠️ Position recovery failed: {}", e);
                // If position recovery fails, we should be conservative and assume we might have a position
                // This prevents executing trades when we're unsure of our state
                if self.current_position.is_none() {
                    warn!("🚫 Position state unclear - skipping signal analysis to prevent duplicate trades");
                    return Ok(());
                }
            }
        }
        
        // Step 1: Check if we have enough data
        let data_points = self.get_data_point_count().await?;
        if data_points < self.config.min_data_points {
            let progress = (data_points as f64 / self.config.min_data_points as f64 * 100.0).min(100.0);
            let progress_bar = self.create_progress_bar(progress);
            
            // Calculate time estimates
            let remaining_points = self.config.min_data_points - data_points;
            let estimated_minutes = remaining_points as f64 / 2.0; // Assuming ~2 data points per minute
            let estimated_seconds = (estimated_minutes * 60.0) as u64;
            
            // Show detailed progress info
            info!("");
            info!("🔄 Data Collection Status:");
            info!("  📊 Progress: {}/{} ({:.1}%) {}", 
                  data_points, self.config.min_data_points, progress, progress_bar);
            info!("  ⏱️  Time Remaining: {:.0}m {:.0}s", 
                  estimated_minutes.floor(), estimated_seconds % 60);
            info!("  📈 Data Rate: ~2 points/minute");
            info!("  🎯 Target: {} points for reliable analysis", self.config.min_data_points);
            
            if data_points > 0 {
                let completion_percentage = (data_points as f64 / self.config.min_data_points as f64 * 100.0).min(100.0);
                if completion_percentage > 50.0 {
                    info!("  🚀 Good progress! Keep collecting data...");
                } else if completion_percentage > 25.0 {
                    info!("  📈 Making steady progress...");
                } else {
                    info!("  🔄 Just getting started...");
                }
            }
            info!("");
            return Ok(());
        }

        // Step 2: Fetch price history
        let prices = self.fetch_price_history().await?;
        if prices.is_empty() {
            warn!("⚠️  No price data available");
            return Ok(());
        }

        // Step 3: Calculate strategy indicators from price data
        let strategy_indicators = self.strategy.calculate_custom_indicators(&prices);

        // Step 3.5: Calculate and post consolidated technical indicators for dashboard
        let consolidated_indicators = self.calculate_consolidated_indicators(&prices, &strategy_indicators);
        if let Err(e) = self.post_consolidated_indicators(&consolidated_indicators).await {
            warn!("Failed to post consolidated indicators: {}", e);
        }

        // Step 3.6: Post individual indicators for backward compatibility
        if let Err(e) = self.post_all_indicators(&consolidated_indicators).await {
            warn!("Failed to post individual indicators: {}", e);
        }

        // Step 4: Analyze and generate signal
        let signal = self.strategy.analyze(&prices, &consolidated_indicators);

        // Step 4.5: STRICT position validation (no cooldown needed)
        let now = Utc::now();
        
        // CRITICAL: Double-check position state before any signal execution
        let has_position = self.current_position.is_some();
        info!("🔍 Signal validation - Position state: {}", if has_position { "ACTIVE" } else { "NONE" });
        
        // STRICT RULE: No BUY signals if we already have a position
        if signal.signal_type == SignalType::Buy && has_position {
            info!("🚫 BLOCKED: BUY signal ignored - already have a position");
            info!("📊 Current position: {:?} at ${:.4}", 
                  self.current_position.as_ref().unwrap().position_type,
                  self.current_position.as_ref().unwrap().entry_price);
            
            // Still post the signal to database for monitoring, but don't execute
            if let Err(e) = self.post_trading_signal(&signal).await {
                warn!("Failed to post signal: {}", e);
            }
            
            // Log the analysis but skip execution
            self.log_analysis(&signal, &prices, &consolidated_indicators);
            return Ok(());
        }
        
        // Step 4.6: Post trading signal to database
        if let Err(e) = self.post_trading_signal(&signal).await {
            warn!("Failed to post signal: {}", e);
        }

        // Step 5: Execute trading logic
        self.execute_signal(&signal).await?;
        
        // Step 6: Log the analysis
        self.log_analysis(&signal, &prices, &consolidated_indicators);

        Ok(())
    }

    async fn get_data_point_count(&self) -> Result<usize> {
        use urlencoding::encode;
        let url = format!("{}/prices/{}", self.config.database_url, encode(&self.config.trading_pair));
        
        let response = self.client.get(&url).send().await?;
        let text = response.text().await?;
        debug!("Raw data point count response: {}", text);
        if text.trim().is_empty() {
            warn!("Data point count endpoint returned empty response");
            return Ok(0);
        }
        let api_response: Result<crate::models::ApiResponse<Vec<PriceFeed>>, _> = serde_json::from_str(&text);
        match api_response {
            Ok(crate::models::ApiResponse { success: true, data: Some(prices), .. }) => {
                info!("📊 API Response - Success: {}, Data points: {}", true, prices.len());
                Ok(prices.len())
            }
            Ok(crate::models::ApiResponse { success: false, error: Some(e), .. }) => {
                warn!("⚠️ API call failed: {}", e);
                Ok(0)
            }
            _ => {
                warn!("⚠️ Unexpected API response format");
                Ok(0)
            }
        }
    }

    async fn fetch_price_history(&self) -> Result<Vec<PriceFeed>> {
        use urlencoding::encode;
        // Try to fetch 1-minute candles first for better analysis
        let candle_url = format!("{}/candles/{}/1m?limit=200", 
                                self.config.database_url, 
                                encode(&self.config.trading_pair));
        
        let response = self.client.get(&candle_url).send().await?;
        let text = response.text().await?;
        debug!("Raw candle response: {}", text);
        if text.trim().is_empty() {
            warn!("Candle endpoint returned empty response");
        }
        let api_response: Result<crate::models::ApiResponse<Vec<crate::models::Candle>>, _> = serde_json::from_str(&text);
        if let Ok(api_response) = api_response {
            match api_response {
                crate::models::ApiResponse { success: true, data: Some(candles), .. } => {
                    if !candles.is_empty() {
                        info!("📊 Using {} 1-minute candles for analysis", candles.len());
                        
                        // Log candle details for debugging
                        if let Some(latest_candle) = candles.first() {
                            info!("🕯️  Latest candle: O={:.4}, H={:.4}, L={:.4}, C={:.4}, Time={}", 
                                  latest_candle.open, latest_candle.high, latest_candle.low, 
                                  latest_candle.close, latest_candle.timestamp.format("%H:%M:%S"));
                        }
                        
                        if candles.len() >= 2 {
                            let prev_candle = &candles[1];
                            info!("🕯️  Previous candle: O={:.4}, H={:.4}, L={:.4}, C={:.4}, Time={}", 
                                  prev_candle.open, prev_candle.high, prev_candle.low, 
                                  prev_candle.close, prev_candle.timestamp.format("%H:%M:%S"));
                        }
                        
                        let prices: Vec<PriceFeed> = candles.into_iter().map(|candle| PriceFeed {
                            id: candle.id,
                            source: "candle".to_string(),
                            pair: candle.pair,
                            price: candle.close,
                            timestamp: candle.timestamp,
                        }).collect();
                        return Ok(prices);
                    }
                }
                _ => {
                    debug!("No candle data available, falling back to raw prices");
                }
            }
        } else {
            warn!("Failed to parse candle response as JSON");
        }
        // Fallback to raw price data if candles are not available
        let url = format!("{}/prices/{}", self.config.database_url, encode(&self.config.trading_pair));
        let response = self.client.get(&url).send().await?;
        let text = response.text().await?;
        debug!("Raw price response: {}", text);
        if text.trim().is_empty() {
            warn!("Price endpoint returned empty response");
            return Ok(vec![]);
        }
        let api_response: Result<crate::models::ApiResponse<Vec<PriceFeed>>, _> = serde_json::from_str(&text);
        match api_response {
            Ok(crate::models::ApiResponse { success: true, data: Some(prices), .. }) => {
                info!("📊 Using {} raw price records for analysis", prices.len());
                Ok(prices)
            }
            Ok(crate::models::ApiResponse { success: false, error: Some(e), .. }) => {
                Err(anyhow::anyhow!("API error: {}", e))
            }
            _ => {
                Err(anyhow::anyhow!("Unexpected or invalid response format"))
            }
        }
    }

    async fn fetch_technical_indicators(&self) -> Result<TechnicalIndicators> {
        use urlencoding::encode;
        let url = format!(
            "{}/indicators/{}?hours=24",
            self.config.database_url,
            encode(&self.config.trading_pair)
        );

        let response = self.client.get(&url).send().await?;
        let text = response.text().await?;
        debug!("Raw technical indicators response: {}", text);
        if text.trim().is_empty() {
            warn!("Technical indicators endpoint returned empty response");
            return Err(anyhow!("Empty response from technical indicators endpoint"));
        }
        let api_response: Result<crate::models::ApiResponse<TechnicalIndicators>, _> = serde_json::from_str(&text);
        match api_response {
            Ok(crate::models::ApiResponse { success: true, data: Some(indicators), .. }) => {
                Ok(indicators)
            }
            Ok(crate::models::ApiResponse { success: false, error: Some(e), .. }) => {
                Err(anyhow!("Failed to fetch technical indicators: {}", e))
            }
            _ => {
                Err(anyhow!("Unexpected response format"))
            }
        }
    }

    async fn execute_signal(&mut self, signal: &TradingSignal) -> Result<()> {
        match signal.signal_type {
            SignalType::Buy => {
                // Double safety check - should never reach here if we already have a position
                if self.current_position.is_none() {
                    info!("🟢 BUY signal detected - no current position, executing trade...");
                    // Execute the trade using trading executor
                    match self.trading_executor.execute_signal(signal, None).await {
                        Ok((true, quantity)) => {
                            // Trade executed successfully (or paper trading)
                            let actual_quantity = quantity.unwrap_or(1.0); // Default to 1.0 if no quantity available
                            self.open_position(signal.price, PositionType::Long, actual_quantity).await?;
                            
                            // Post position to database - CRITICAL: Must succeed
                            if let Some(position) = &self.current_position {
                                match self.post_position(position, signal.take_profit, signal.stop_loss).await {
                                    Ok(_) => {
                                        info!("");
                                        info!("🟢 BUY SIGNAL EXECUTED");
                                        info!("═══════════════════════════════════════════════════════════════");
                                        info!("  💰 Entry Price: ${:.4}", signal.price);
                                        info!("  🎯 Confidence: {:.1}%", signal.confidence * 100.0);
                                        info!("  📊 Position Type: Long");
                                        info!("  🎯 Dynamic Take Profit: {:.2}%", signal.take_profit * 100.0);
                                        info!("  🛑 Dynamic Stop Loss: {:.2}%", signal.stop_loss * 100.0);
                                        info!("  ⏰ Timestamp: {}", Utc::now().format("%Y-%m-%d %H:%M:%S UTC"));
                                        info!("═══════════════════════════════════════════════════════════════");
                                        info!("");
                                    }
                                    Err(e) => {
                                        error!("❌ CRITICAL: Transaction succeeded but database logging failed: {}", e);
                                        error!("🚫 Rolling back position in memory to prevent inconsistency");
                                        // Roll back the position in memory since database logging failed
                                        self.current_position = None;
                                        return Err(anyhow!("Database logging failed after successful transaction: {}", e));
                                    }
                                }
                            } else {
                                error!("❌ CRITICAL: Position opened in memory but position is None");
                                return Err(anyhow!("Position opened but position is None"));
                            }
                        }
                        Ok((false, _)) => {
                            warn!("⚠️  BUY signal execution failed or was skipped");
                        }
                        Err(e) => {
                            error!("❌ BUY signal execution error: {}", e);
                        }
                    }
                } else {
                    error!("🚫 CRITICAL: BUY signal reached execute_signal with existing position - this should never happen!");
                    info!("📊 Current position: {:?} at ${:.4}", 
                          self.current_position.as_ref().unwrap().position_type,
                          self.current_position.as_ref().unwrap().entry_price);
                    info!("🛑 Blocking execution to prevent multiple positions");
                }
            }
            SignalType::Sell => {
                if let Some(position) = &self.current_position {
                    // Extract position data before mutable borrow
                    let entry_price = position.entry_price;
                    let entry_time = position.entry_time;
                    let position_type = position.position_type.clone();
                    let position_quantity = position.quantity;
                    
                    // Execute the trade using trading executor
                    match self.trading_executor.execute_signal(signal, Some(position_quantity)).await {
                        Ok((true, _)) => {
                            // Trade executed successfully (or paper trading)
                            let pnl = self.calculate_pnl(signal.price, position);
                            let duration = Utc::now() - entry_time;
                            self.close_position(signal.price).await?;
                            

                            
                            let pnl_emoji = if pnl > 0.0 { "💰" } else if pnl < 0.0 { "💸" } else { "➡️" };
                            let pnl_status = if pnl > 0.0 { "PROFIT" } else if pnl < 0.0 { "LOSS" } else { "BREAKEVEN" };
                            
                            info!("");
                            info!("🔴 SELL SIGNAL EXECUTED - {}", pnl_status);
                            info!("═══════════════════════════════════════════════════════════════");
                            info!("  💰 Exit Price: ${:.4}", signal.price);
                            info!("  📈 Entry Price: ${:.4}", entry_price);
                            info!("  {} PnL: {:.2}%", pnl_emoji, pnl * 100.0);
                            info!("  🎯 Confidence: {:.1}%", signal.confidence * 100.0);
                            info!("  ⏱️  Duration: {}s", duration.num_seconds());
                            info!("  📊 Position Type: {:?}", position_type);
                            info!("  ⏰ Timestamp: {}", Utc::now().format("%Y-%m-%d %H:%M:%S UTC"));
                            info!("═══════════════════════════════════════════════════════════════");
                            info!("");
                        }
                        Ok((false, _)) => {
                            warn!("⚠️  SELL signal execution failed or was skipped");
                        }
                        Err(e) => {
                            error!("❌ SELL signal execution error: {}", e);
                        }
                    }
                } else {
                    debug!("No position to sell");
                }
            }
            SignalType::Hold => {
                // Check for stop loss or take profit using dynamic thresholds
                if let Some(position) = &self.current_position {
                    let pnl = self.calculate_pnl(signal.price, position);
                    
                    // Minimum hold time check (5 minutes = 300 seconds)
                    let duration = Utc::now() - position.entry_time;
                    let min_hold_time_seconds = 300; // 5 minutes minimum hold
                    
                    if duration.num_seconds() < min_hold_time_seconds {
                        debug!("⏳ Position held for {}s, minimum hold time is {}s - waiting for profit", 
                               duration.num_seconds(), min_hold_time_seconds);
                        return Ok(());
                    }
                    
                    // Enhanced exit conditions for quick swaps
                    let should_exit = self.check_enhanced_exit_conditions(signal, position, pnl).await?;
                    
                    if should_exit {
                        let entry_price = position.entry_price;
                        let entry_time = position.entry_time;
                        let position_type = position.position_type.clone();
                        let position_quantity = position.quantity;
                        
                        // Execute the sell transaction first
                        match self.trading_executor.execute_signal(signal, Some(position_quantity)).await {
                            Ok((true, _)) => {
                                // Trade executed successfully, now close position in database
                                self.close_position(signal.price).await?;
                                
                                info!("");
                                info!("🚀 ENHANCED EXIT TRIGGERED");
                                info!("═══════════════════════════════════════════════════════════════");
                                info!("  💰 Exit Price: ${:.4}", signal.price);
                                info!("  📈 Entry Price: ${:.4}", entry_price);
                                info!("  💰 PnL: {:.2}%", pnl * 100.0);
                                info!("  ⏱️  Duration: {}s", duration.num_seconds());
                                info!("  ⏰ Timestamp: {}", Utc::now().format("%Y-%m-%d %H:%M:%S UTC"));
                                info!("═══════════════════════════════════════════════════════════════");
                                info!("");
                            }
                            Ok((false, _)) => {
                                warn!("⚠️  ENHANCED EXIT signal execution failed or was skipped");
                            }
                            Err(e) => {
                                error!("❌ ENHANCED EXIT signal execution error: {}", e);
                            }
                        }
                    }
                }
            }
        }

        Ok(())
    }

    async fn open_position(&mut self, price: f64, position_type: PositionType, quantity: f64) -> Result<()> {
        // Safety check: Ensure we don't already have a position
        if self.current_position.is_some() {
            warn!("🚫 SAFETY CHECK FAILED: Attempted to open position when one already exists!");
            warn!("📊 Existing position: {:?} at ${:.4}", 
                  self.current_position.as_ref().unwrap().position_type,
                  self.current_position.as_ref().unwrap().entry_price);
            warn!("🎯 Attempted to open: {:?} at ${:.4}", position_type, price);
            return Err(anyhow!("Cannot open position - one already exists"));
        }
        
        let log_type = position_type.clone();
        let position = Position {
            position_id: None,
            entry_price: price,
            entry_time: Utc::now(),
            quantity, // Use the actual quantity received from the transaction
            position_type,
        };
        
        // Post position to database and get the position ID
        let position_id = self.post_position(&position, 0.05, 0.03).await?;
        
        // Update the position with the ID from database
        let mut final_position = position;
        final_position.position_id = Some(position_id.clone());
        
        self.current_position = Some(final_position);
        
        info!("📈 Opened {:?} position at ${:.4} with quantity {:.6}", log_type, price, quantity);
        info!("🔒 Position safety check passed - no duplicate positions");
        info!("🆔 Database position ID: {}", position_id.clone());
        Ok(())
    }

    async fn close_position(&mut self, price: f64) -> Result<()> {
        if let Some(position) = &self.current_position {
            let pnl = self.calculate_pnl(price, position);
            let duration = Utc::now() - position.entry_time;
            
            info!("📉 Closed position at ${:.4} - PnL: {:.2}% (Duration: {}s)", 
                  price, pnl * 100.0, duration.num_seconds());
            
            // Close position in database - CRITICAL: Must succeed before clearing memory
            match self.close_position_in_database(price).await {
                Ok(_) => {
                    info!("✅ Successfully closed position in database, clearing from memory");
                    self.current_position = None;
                }
                Err(e) => {
                    error!("❌ CRITICAL: Failed to close position in database: {}", e);
                    error!("🚫 Keeping position in memory to prevent inconsistency");
                    error!("🔄 Position will be retried on next cycle");
                    return Err(anyhow!("Database close failed: {}", e));
                }
            }
        } else {
            warn!("⚠️  Attempted to close position but no position in memory");
        }
        
        Ok(())
    }

    fn calculate_pnl(&self, current_price: f64, position: &Position) -> f64 {
        match position.position_type {
            PositionType::Long => (current_price - position.entry_price) / position.entry_price,
            PositionType::Short => (position.entry_price - current_price) / position.entry_price,
        }
    }

    fn log_analysis(&self, signal: &TradingSignal, prices: &[PriceFeed], indicators: &TechnicalIndicators) {
        let price_count = prices.len();
        let current_price = signal.price;
        
        // Get latest price change
        let price_change = if prices.len() >= 2 {
            let current = prices.last().unwrap().price;
            let previous = prices[prices.len() - 2].price;
            let change = ((current - previous) / previous) * 100.0;
            Some(change)
        } else {
            None
        };
        
        info!("");
        info!("🎯 Enhanced Trading Analysis Report");
        info!("═══════════════════════════════════════════════════════════════");
        info!("  💰 Current Price: ${:.4}", current_price);
        
        if let Some(change) = price_change {
            let change_emoji = if change > 0.0 { "📈" } else if change < 0.0 { "📉" } else { "➡️" };
            info!("  {} Price Change: {:.3}%", change_emoji, change);
        }
        
        info!("  📊 Data Points: {} | Signal: {:?}", price_count, signal.signal_type);
        info!("  🎯 Confidence: {:.1}%", signal.confidence * 100.0);
        
        // Log data source information
        if !prices.is_empty() {
            let data_source = prices.first().unwrap().source.as_str();
            if data_source == "candle" {
                info!("  🕯️  Data Source: 1-minute candles (OHLC)");
                if prices.len() >= 2 {
                    let latest = prices.last().unwrap();
                    let previous = &prices[prices.len() - 2];
                    let candle_range = ((latest.price - previous.price) / previous.price) * 100.0;
                    info!("  📈 Candle Range: {:.3}% (${:.4} → ${:.4})", 
                          candle_range, previous.price, latest.price);
                }
            } else {
                info!("  📊 Data Source: Raw price data");
            }
        }
        
        // Position status with enhanced information
        if let Some(position) = &self.current_position {
            let pnl = self.calculate_pnl(current_price, position);
            let duration = Utc::now() - position.entry_time;
            let pnl_emoji = if pnl > 0.0 { "🟢" } else if pnl < 0.0 { "🔴" } else { "🟡" };
            
            info!("  📈 Active Position: {:?} | Entry: ${:.4}", position.position_type, position.entry_price);
            info!("  {} Unrealized PnL: {:.2}% | Duration: {}s", pnl_emoji, pnl * 100.0, duration.num_seconds());
            
            // Calculate distance to take profit and stop loss
            let tp_distance = if pnl > 0.0 { signal.take_profit - pnl } else { signal.take_profit };
            let sl_distance = if pnl < 0.0 { signal.stop_loss + pnl.abs() } else { signal.stop_loss };
            
            info!("  🎯 Take Profit: {:.2}% away | Stop Loss: {:.2}% away", 
                  tp_distance * 100.0, sl_distance * 100.0);
        } else {
            info!("  💤 No active position");
        }
        
        // Multi-timeframe technical indicators
        info!("");
        info!("📈 Multi-Timeframe Technical Indicators:");
        
        // Short-term indicators (30 minutes)
        let short_term_prices = self.get_recent_prices(&prices, 30 * 60);
        if short_term_prices.len() >= 20 {
            let short_indicators = self.strategy.calculate_custom_indicators(&short_term_prices);
            info!("  ⚡ Short-term (30m):");
            if let Some(rsi) = short_indicators.rsi_fast {
                let rsi_status = if rsi > 70.0 { "🔴 Overbought" } else if rsi < 30.0 { "🟢 Oversold" } else { "🟡 Neutral" };
                info!("    📊 RSI: {:.2} {}", rsi, rsi_status);
            }
            if let Some(sma) = short_indicators.sma_short {
                let sma_status = if current_price > sma { "📈 Above" } else { "📉 Below" };
                info!("    📈 SMA20: {:.4} {}", sma, sma_status);
            }
            if let Some(vol) = short_indicators.volatility {
                let vol_status = if vol > 0.05 { "🔥 High" } else if vol > 0.02 { "⚡ Medium" } else { "❄️ Low" };
                info!("    📊 Volatility: {:.2}% {}", vol * 100.0, vol_status);
            }
        }
        
        // Medium-term indicators (2 hours)
        let medium_term_prices = self.get_recent_prices(prices, 2 * 60 * 60);
        if medium_term_prices.len() >= 50 {
            let medium_indicators = self.strategy.calculate_custom_indicators(&medium_term_prices);
            info!("  📊 Medium-term (2h):");
            if let Some(rsi) = medium_indicators.rsi_fast {
                let rsi_status = if rsi > 70.0 { "🔴 Overbought" } else if rsi < 30.0 { "🟢 Oversold" } else { "🟡 Neutral" };
                info!("    📊 RSI: {:.2} {}", rsi, rsi_status);
            }
            if let Some(sma) = medium_indicators.sma_long {
                let sma_status = if current_price > sma { "📈 Above" } else { "📉 Below" };
                info!("    📈 SMA50: {:.4} {}", sma, sma_status);
            }
        }
        
        // Long-term indicators (6 hours)
        let long_term_prices = self.get_recent_prices(prices, 6 * 60 * 60);
        if long_term_prices.len() >= 100 {
            let long_indicators = self.strategy.calculate_custom_indicators(&long_term_prices);
            info!("  📈 Long-term (6h):");
            if let Some(rsi) = long_indicators.rsi_fast {
                let rsi_status = if rsi > 70.0 { "🔴 Overbought" } else if rsi < 30.0 { "🟢 Oversold" } else { "🟡 Neutral" };
                info!("    📊 RSI: {:.2} {}", rsi, rsi_status);
            }
            if let Some(sma) = long_indicators.sma_long {
                let sma_status = if current_price > sma { "📈 Above" } else { "📉 Below" };
                info!("    📈 SMA50: {:.4} {}", sma, sma_status);
            }
        }
        
        // Current timeframe indicators (from database)
        info!("  📊 Current timeframe:");
        if let Some(rsi) = indicators.rsi_14 {
            let rsi_status = if rsi > 70.0 { "🔴 Overbought" } else if rsi < 30.0 { "�� Oversold" } else { "🟡 Neutral" };
            info!("    📊 RSI (14): {:.2} {}", rsi, rsi_status);
        }
        if let Some(sma_20) = indicators.sma_20 {
            let sma_status = if current_price > sma_20 { "📈 Above" } else { "📉 Below" };
            info!("    📈 SMA (20): {:.4} {}", sma_20, sma_status);
        }
        if let Some(sma_50) = indicators.sma_50 {
            let sma_status = if current_price > sma_50 { "📈 Above" } else { "📉 Below" };
            info!("    📈 SMA (50): {:.4} {}", sma_50, sma_status);
        }
        if let Some(volatility) = indicators.volatility_24h {
            let vol_status = if volatility > 0.05 { "🔥 High" } else if volatility > 0.02 { "⚡ Medium" } else { "❄️ Low" };
            info!("    📊 Volatility (24h): {:.2}% {}", volatility * 100.0, vol_status);
        }
        if let Some(price_change_24h) = indicators.price_change_24h {
            let change_emoji = if price_change_24h > 0.0 { "📈" } else if price_change_24h < 0.0 { "📉" } else { "➡️" };
            info!("    {} 24h Change: {:.2}%", change_emoji, price_change_24h * 100.0);
        }
        
        // Market regime and trend analysis
        info!("");
        info!("🎭 Market Analysis:");
        
        // Calculate market regime
        let (market_regime, trend_strength) = self.analyze_market_regime(prices);
        let regime_emoji = match market_regime.as_str() {
            "Trending" => "📈",
            "Ranging" => "🔄", 
            "Volatile" => "⚡",
            "Consolidating" => "🦀",
            _ => "❓"
        };
        info!("  {} Market Regime: {} (Strength: {:.1}%)", regime_emoji, market_regime, trend_strength * 100.0);
        
        // Support and resistance levels
        let (support, resistance) = self.calculate_support_resistance(prices);
        if let Some(support_level) = support {
            let support_distance = ((current_price - support_level) / current_price) * 100.0;
            let support_emoji = if support_distance < 2.0 { "🟢" } else if support_distance < 5.0 { "🟡" } else { "🔴" };
            info!("  {} Support Level: ${:.4} ({:.1}% away)", support_emoji, support_level, support_distance);
        }
        if let Some(resistance_level) = resistance {
            let resistance_distance = ((resistance_level - current_price) / current_price) * 100.0;
            let resistance_emoji = if resistance_distance < 2.0 { "🟢" } else if resistance_distance < 5.0 { "🟡" } else { "🔴" };
            info!("  {} Resistance Level: ${:.4} ({:.1}% away)", resistance_emoji, resistance_level, resistance_distance);
        }
        
        // Dynamic thresholds
        let dynamic_thresholds = self.calculate_dynamic_thresholds(prices);
        info!("  🎯 Dynamic Thresholds:");
        info!("    RSI Oversold: {:.1} | RSI Overbought: {:.1}", 
              dynamic_thresholds.rsi_oversold, dynamic_thresholds.rsi_overbought);
        info!("    Take Profit: {:.2}% | Stop Loss: {:.2}%", 
              dynamic_thresholds.take_profit * 100.0, dynamic_thresholds.stop_loss * 100.0);
        info!("    Momentum Threshold: {:.2}% | Volatility Multiplier: {:.1}x", 
              dynamic_thresholds.momentum_threshold * 100.0, dynamic_thresholds.volatility_multiplier);
        
        // Enhanced signal reasoning
        info!("");
        info!("🧠 Enhanced Signal Analysis:");
        if signal.reasoning.is_empty() {
            info!("  💭 No specific reasoning available");
        } else {
            for (i, reason) in signal.reasoning.iter().enumerate() {
                info!("  {}. {}", i + 1, reason);
            }
        }
        
        // Risk assessment
        info!("");
        info!("⚠️ Risk Assessment:");
        let volatility_risk = indicators.volatility_24h.unwrap_or(0.02);
        let risk_level = if volatility_risk > 0.08 { "🔴 High" } else if volatility_risk > 0.04 { "🟡 Medium" } else { "🟢 Low" };
        info!("  📊 Volatility Risk: {} ({:.2}%)", risk_level, volatility_risk * 100.0);
        
        if let Some(position) = &self.current_position {
            let pnl = self.calculate_pnl(current_price, position);
            let risk_reward_ratio = if pnl > 0.0 { signal.take_profit / pnl.abs() } else { signal.take_profit / signal.stop_loss };
            info!("  ⚖️ Risk/Reward Ratio: {:.2}:1", risk_reward_ratio);
            
            let max_drawdown_potential = signal.stop_loss * 100.0;
            info!("  📉 Max Drawdown Potential: {:.2}%", max_drawdown_potential);
        }
        
        // Market sentiment based on multiple indicators
        let mut bullish_signals = 0;
        let mut bearish_signals = 0;
        
        if let Some(rsi) = indicators.rsi_14 {
            if rsi < 30.0 { bullish_signals += 1; }
            if rsi > 70.0 { bearish_signals += 1; }
        }
        if let Some(sma_20) = indicators.sma_20 {
            if current_price > sma_20 { bullish_signals += 1; }
            if current_price < sma_20 { bearish_signals += 1; }
        }
        if let Some(sma_50) = indicators.sma_50 {
            if current_price > sma_50 { bullish_signals += 1; }
            if current_price < sma_50 { bearish_signals += 1; }
        }
        
        let sentiment = if bullish_signals > bearish_signals { "🐂 Bullish" } 
                       else if bearish_signals > bullish_signals { "🐻 Bearish" } 
                       else { "🦀 Sideways" };
        
        info!("  🎭 Market Sentiment: {} ({} bullish, {} bearish signals)", 
              sentiment, bullish_signals, bearish_signals);
        
        // Performance context
        info!("");
        info!("📊 Performance Context:");
        let signal_strength = if signal.confidence > 0.7 { "🟢 Strong" } else if signal.confidence > 0.5 { "🟡 Moderate" } else { "🔴 Weak" };
        info!("  🎯 Signal Strength: {} ({:.1}%)", signal_strength, signal.confidence * 100.0);
        
        let market_condition = match market_regime.as_str() {
            "Trending" => "📈 Favorable for trend following",
            "Ranging" => "🔄 Favorable for mean reversion", 
            "Volatile" => "⚡ High risk, high reward",
            "Consolidating" => "🦀 Low volatility, wait for breakout",
            _ => "❓ Unknown market condition"
        };
        info!("  🌍 Market Condition: {}", market_condition);
        
        info!("═══════════════════════════════════════════════════════════════");
        info!("");
    }

    fn calculate_consolidated_indicators(&self, prices: &[PriceFeed], strategy_indicators: &crate::models::TradingIndicators) -> crate::models::TechnicalIndicators {
        let current_price = prices.first().map(|p| p.price).unwrap_or(0.0);
        let now = Utc::now();
        
        // Calculate RSI14 (dashboard expects RSI14, not RSI7/21)
        let rsi_14 = if prices.len() >= 14 {
            let price_values: Vec<f64> = prices.iter().map(|p| p.price).collect();
            self.calculate_rsi_14(&price_values)
        } else {
            None
        };
        
        // Calculate SMA20 and SMA50 (already calculated by strategy)
        let sma_20 = strategy_indicators.sma_short;
        let sma_50 = strategy_indicators.sma_long;
        
        // Calculate SMA200 (dashboard expects this)
        let sma_200 = if prices.len() >= 200 {
            let price_values: Vec<f64> = prices.iter().map(|p| p.price).collect();
            self.calculate_sma_200(&price_values)
        } else {
            None
        };
        
        // Calculate 24h price change (dashboard expects this)
        let (price_change_24h, price_change_percent_24h) = if prices.len() >= 24 * 60 { // 24 hours of minute data
            let current = prices[prices.len() - 1].price;
            let past_24h = prices[prices.len() - 24 * 60].price;
            let change = current - past_24h;
            let change_percent = (change / past_24h) * 100.0;
            (Some(change), Some(change_percent))
        } else {
            (None, None)
        };
        
        // Calculate 24h volatility (dashboard expects this)
        let volatility_24h = if prices.len() >= 24 * 60 {
            let price_values: Vec<f64> = prices.iter().map(|p| p.price).collect();
            self.calculate_volatility_24h(&price_values)
        } else {
            None
        };
        
        crate::models::TechnicalIndicators {
            pair: self.config.trading_pair.clone(),
            timestamp: now,
            sma_20,
            sma_50,
            sma_200,
            rsi_14,
            price_change_24h,
            price_change_percent_24h,
            volatility_24h,
            current_price,
        }
    }

    fn calculate_rsi_14(&self, prices: &[f64]) -> Option<f64> {
        if prices.len() < 14 {
            return None;
        }
        
        let mut gains = 0.0;
        let mut losses = 0.0;
        
        // Calculate initial average gain and loss
        for i in 1..14 {
            let change = prices[i] - prices[i - 1];
            if change > 0.0 {
                gains += change;
            } else {
                losses += change.abs();
            }
        }
        
        let avg_gain = gains / 14.0;
        let avg_loss = losses / 14.0;
        
        if avg_loss == 0.0 {
            return Some(100.0);
        }
        
        let rs = avg_gain / avg_loss;
        let rsi = 100.0 - (100.0 / (1.0 + rs));
        
        Some(rsi)
    }
    
    fn calculate_sma_200(&self, prices: &[f64]) -> Option<f64> {
        if prices.len() < 200 {
            return None;
        }
        
        let sum: f64 = prices[prices.len() - 200..].iter().sum();
        Some(sum / 200.0)
    }
    
    fn calculate_volatility_24h(&self, prices: &[f64]) -> Option<f64> {
        if prices.len() < 24 * 60 { // Need 24 hours of minute data
            return None;
        }
        
        let recent_prices = &prices[prices.len() - 24 * 60..];
        let returns: Vec<f64> = recent_prices.windows(2)
            .map(|window| (window[1] - window[0]) / window[0])
            .collect();
        
        let mean_return = returns.iter().sum::<f64>() / returns.len() as f64;
        let variance = returns.iter()
            .map(|r| (r - mean_return).powi(2))
            .sum::<f64>() / returns.len() as f64;
        
        Some(variance.sqrt())
    }

    // Database API methods for posting data
    async fn post_consolidated_indicators(&self, indicators: &crate::models::TechnicalIndicators) -> Result<()> {
        let url = format!("{}/indicators/{}/store", self.config.database_url, 
                         urlencoding::encode(&self.config.trading_pair));
        
        // Convert to the format expected by the database service
        let store_request = crate::models::StoreTechnicalIndicatorsRequest {
            pair: self.config.trading_pair.clone(),
            sma_20: indicators.sma_20,
            sma_50: indicators.sma_50,
            sma_200: indicators.sma_200,
            rsi_14: indicators.rsi_14,
            price_change_24h: indicators.price_change_24h,
            price_change_percent_24h: indicators.price_change_percent_24h,
            volatility_24h: indicators.volatility_24h,
            current_price: indicators.current_price,
        };
        
        let response = self.client.post(&url)
            .json(&store_request)
            .send()
            .await?;
            
        if !response.status().is_success() {
            warn!("Failed to post consolidated technical indicators: {}", response.status());
        } else {
            debug!("Posted consolidated technical indicators: RSI14={:?}, SMA20={:?}, SMA50={:?}, Volatility24h={:?}", 
                   indicators.rsi_14, indicators.sma_20, indicators.sma_50, indicators.volatility_24h);
        }
        
        Ok(())
    }

    async fn post_technical_indicator(&self, indicator: &TechnicalIndicator) -> Result<()> {
        let url = format!("{}/indicators/{}/store", self.config.database_url, 
                         urlencoding::encode(&indicator.pair));
        
        // Convert to the format expected by the database service
        let store_request = crate::models::StoreTechnicalIndicatorsRequest {
            pair: indicator.pair.clone(),
            sma_20: if indicator.indicator_type == "SMA" && indicator.period == Some(20) { Some(indicator.value) } else { None },
            sma_50: if indicator.indicator_type == "SMA" && indicator.period == Some(50) { Some(indicator.value) } else { None },
            sma_200: if indicator.indicator_type == "SMA" && indicator.period == Some(200) { Some(indicator.value) } else { None },
            rsi_14: if indicator.indicator_type == "RSI" { Some(indicator.value) } else { None },
            price_change_24h: None,
            price_change_percent_24h: None,
            volatility_24h: if indicator.indicator_type == "Volatility" { Some(indicator.value) } else { None },
            current_price: indicator.value,
        };
        
        let response = self.client.post(&url)
            .json(&store_request)
            .send()
            .await?;
            
        if !response.status().is_success() {
            warn!("Failed to post technical indicator: {}", response.status());
        } else {
            debug!("Posted technical indicator: {} = {:.4}", indicator.indicator_type, indicator.value);
        }
        
        Ok(())
    }

    async fn post_trading_signal(&self, signal: &TradingSignal) -> Result<()> {
        let signal_db = TradingSignalDb {
            pair: self.config.trading_pair.clone(),
            timestamp: signal.timestamp,
            signal_type: match signal.signal_type {
                SignalType::Buy => "buy".to_string(),
                SignalType::Sell => "sell".to_string(),
                SignalType::Hold => "hold".to_string(),
            },
            confidence: signal.confidence,
            price: signal.price,
            reasoning: signal.reasoning.join("; "),
            take_profit: signal.take_profit,
            stop_loss: signal.stop_loss,
        };

        let url = format!("{}/signals", self.config.database_url);
        
        let response = self.client.post(&url)
            .json(&signal_db)
            .send()
            .await?;
            
        if !response.status().is_success() {
            warn!("Failed to post trading signal: {}", response.status());
        } else {
            debug!("Posted trading signal: {:?} (confidence: {:.1}%)", signal.signal_type, signal.confidence * 100.0);
        }
        
        Ok(())
    }

    async fn post_position(&self, position: &Position, take_profit: f64, stop_loss: f64) -> Result<String> {
        // Get wallet address from Solana private key
        let wallet_address = self.trading_executor.get_wallet_address()?;

        // Ensure wallet exists in the database
        let create_wallet_request = serde_json::json!({
            "address": wallet_address,
        });
        let wallet_url = format!("{}/wallets", self.config.database_url);
        let wallet_response = self.client.post(&wallet_url)
            .timeout(Duration::from_secs(10))
            .json(&create_wallet_request)
            .send()
            .await?;
        if !wallet_response.status().is_success() {
            warn!("Failed to create wallet: {}", wallet_response.status());
        }
        // Continue regardless of wallet creation result (it may already exist)
        
        // Create the correct request structure that the database service expects
        let create_position_request = serde_json::json!({
            "wallet_address": wallet_address,
            "pair": self.config.trading_pair,
            "position_type": match position.position_type {
                PositionType::Long => "long",
                PositionType::Short => "short",
            },
            "entry_price": position.entry_price,
            "quantity": position.quantity, // Use the actual quantity from the position
        });

        let url = format!("{}/positions", self.config.database_url);
        
        let response = self.client.post(&url)
            .timeout(Duration::from_secs(15))
            .json(&create_position_request)
            .send()
            .await?;
            
        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            let error_msg = format!("Failed to post position to database: {} - {}", status, error_text);
            error!("❌ {}", error_msg);
            return Err(anyhow!(error_msg));
        } else {
            let response_text = response.text().await.unwrap_or_else(|_| "No response body".to_string());
            info!("✅ Successfully posted position to database: {:?} at ${:.4}", position.position_type, position.entry_price);
            debug!("📊 Database response: {}", response_text);
            
            // Parse the response to get the position ID
            match serde_json::from_str::<serde_json::Value>(&response_text) {
                Ok(json_response) => {
                    if let Some(data) = json_response.get("data") {
                        if let Some(position_id) = data.get("id").and_then(|id| id.as_str()) {
                            info!("🆔 Captured position ID: {}", position_id);
                            return Ok(position_id.to_string());
                        }
                    }
                    warn!("⚠️ Could not extract position ID from response");
                    Ok("".to_string()) // Return empty string if we can't get the ID
                }
                Err(e) => {
                    warn!("⚠️ Failed to parse position response: {}", e);
                    Ok("".to_string()) // Return empty string if parsing fails
                }
            }
        }
    }

    async fn post_trade(&self, entry_price: f64, exit_price: f64, entry_time: DateTime<Utc>, 
                       position_type: &PositionType, pnl: f64) -> Result<()> {
        let trade_db = TradeDb {
            pair: self.config.trading_pair.clone(),
            trade_type: match position_type {
                PositionType::Long => "long".to_string(),
                PositionType::Short => "short".to_string(),
            },
            entry_price,
            exit_price,
            quantity: 1.0, // Default quantity
            entry_time,
            exit_time: Utc::now(),
            pnl,
            pnl_percent: pnl * 100.0,
            signal_id: None, // Could be linked to signal ID if available
        };

        let url = format!("{}/trades", self.config.database_url);
        
        let response = self.client.post(&url)
            .json(&trade_db)
            .send()
            .await?;
            
        if !response.status().is_success() {
            warn!("Failed to post trade: {}", response.status());
        } else {
            let pnl_emoji = if pnl > 0.0 { "💰" } else if pnl < 0.0 { "💸" } else { "➡️" };
            debug!("Posted trade: {} PnL: {:.2}%", pnl_emoji, pnl * 100.0);
        }
        
        Ok(())
    }

    async fn post_trading_config(&self) -> Result<()> {
        // Create the correct request structure that the database service expects
        let timestamp = Utc::now().format("%Y%m%d_%H%M%S").to_string();
        let config_name = format!("RSI_Trend_Strategy_{}", timestamp);
        
        let create_config_request = serde_json::json!({
            "name": config_name,
            "pair": self.config.trading_pair,
            "min_data_points": self.config.min_data_points as i32,
            "check_interval_secs": self.config.check_interval_secs as i32,
            "take_profit_percent": self.config.take_profit_threshold * 100.0,
            "stop_loss_percent": self.config.stop_loss_threshold * 100.0,
            "max_position_size": 100.0,
        });

        let url = format!("{}/configs", self.config.database_url);
        
        // Log the request for debugging
        debug!("Sending trading config request: {}", serde_json::to_string_pretty(&create_config_request)?);
        
        let response = self.client.post(&url)
            .json(&create_config_request)
            .send()
            .await?;
            
        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            warn!("Failed to post trading config: {} - {}", status, error_text);
        } else {
            debug!("Posted trading config for {}", self.config.trading_pair);
        }
        
        Ok(())
    }

    async fn post_all_indicators(&self, indicators: &TechnicalIndicators) -> Result<()> {
        let now = Utc::now();
        
        // Post RSI
        if let Some(rsi) = indicators.rsi_14 {
            let rsi_indicator = TechnicalIndicator {
                pair: self.config.trading_pair.clone(),
                timestamp: now,
                indicator_type: "RSI".to_string(),
                value: rsi,
                period: Some(14),
            };
            self.post_technical_indicator(&rsi_indicator).await?;
        }

        // Post SMA indicators
        if let Some(sma_20) = indicators.sma_20 {
            let sma_indicator = TechnicalIndicator {
                pair: self.config.trading_pair.clone(),
                timestamp: now,
                indicator_type: "SMA".to_string(),
                value: sma_20,
                period: Some(20),
            };
            self.post_technical_indicator(&sma_indicator).await?;
        }

        if let Some(sma_50) = indicators.sma_50 {
            let sma_indicator = TechnicalIndicator {
                pair: self.config.trading_pair.clone(),
                timestamp: now,
                indicator_type: "SMA".to_string(),
                value: sma_50,
                period: Some(50),
            };
            self.post_technical_indicator(&sma_indicator).await?;
        }

        if let Some(sma_200) = indicators.sma_200 {
            let sma_indicator = TechnicalIndicator {
                pair: self.config.trading_pair.clone(),
                timestamp: now,
                indicator_type: "SMA".to_string(),
                value: sma_200,
                period: Some(200),
            };
            self.post_technical_indicator(&sma_indicator).await?;
        }

        // Post volatility
        if let Some(volatility) = indicators.volatility_24h {
            let vol_indicator = TechnicalIndicator {
                pair: self.config.trading_pair.clone(),
                timestamp: now,
                indicator_type: "Volatility".to_string(),
                value: volatility,
                period: None,
            };
            self.post_technical_indicator(&vol_indicator).await?;
        }

        Ok(())
    }

    // Position persistence methods
    async fn fetch_open_positions(&self) -> Result<Option<Position>> {
        use urlencoding::encode;
        let encoded_pair = encode(&self.config.trading_pair);
        let url = format!("{}/positions/pair/{}/open", self.config.database_url, encoded_pair);
        
        info!("🔍 Fetching open positions from: {}", url);
        info!("🔍 Trading pair: '{}' -> encoded: '{}'", self.config.trading_pair, encoded_pair);
        
        let response = self.client.get(&url).send().await?;
        let status = response.status();
        let text = response.text().await?;
        
        info!("🔍 Database response status: {}", status);
        info!("🔍 Database response body: {}", text);
        
        if text.trim().is_empty() {
            warn!("Open positions endpoint returned empty response");
            return Ok(None);
        }
        
        let api_response: Result<crate::models::ApiResponse<Option<PositionDb>>, _> = serde_json::from_str(&text);
        match api_response {
            Ok(crate::models::ApiResponse { success: true, data: Some(Some(position_db)), .. }) => {
                info!("✅ Successfully found open position in database");
                let position = Position {
                    position_id: Some(position_db.id.clone()),
                    entry_price: position_db.entry_price,
                    entry_time: position_db.entry_time,
                    quantity: position_db.quantity,
                    position_type: match position_db.position_type.as_str() {
                        "long" => PositionType::Long,
                        "short" => PositionType::Short,
                        _ => return Err(anyhow!("Invalid position type: {}", position_db.position_type)),
                    },
                };
                Ok(Some(position))
            }
            Ok(crate::models::ApiResponse { success: true, data: Some(None), .. }) => {
                info!("💤 Database confirmed no open positions");
                Ok(None)
            }
            Ok(crate::models::ApiResponse { success: true, data: None, .. }) => {
                info!("💤 Database confirmed no open positions (data is None)");
                Ok(None)
            }
            Ok(crate::models::ApiResponse { success: false, error: Some(e), .. }) => {
                warn!("❌ Database error: {}", e);
                Ok(None)
            }
            Ok(crate::models::ApiResponse { success: false, error: None, .. }) => {
                warn!("❌ Database returned success: false with no error message");
                Ok(None)
            }
            Err(e) => {
                warn!("❌ Failed to parse database response: {}", e);
                warn!("❌ Raw response: {}", text);
                Ok(None)
            }
        }
    }

    async fn update_position_status(&self, position_id: &str, status: &str) -> Result<()> {
        let url = format!("{}/positions/{}/status", self.config.database_url, position_id);
        
        let update_data = serde_json::json!({
            "status": status
        });
        
        let response = self.client.patch(&url)
            .json(&update_data)
            .send()
            .await?;
            
        if !response.status().is_success() {
            warn!("Failed to update position status: {}", response.status());
        } else {
            debug!("Updated position {} status to {}", position_id, status);
        }
        
        Ok(())
    }

    async fn recover_positions(&mut self) -> Result<()> {
        info!("🔄 Recovering positions from database...");
        
        match self.fetch_open_positions().await {
            Ok(Some(position)) => {
                self.current_position = Some(position.clone());
                info!("📈 Recovered {} position: Entry ${:.4} at {}", 
                      match position.position_type {
                          PositionType::Long => "Long",
                          PositionType::Short => "Short",
                      },
                      position.entry_price,
                      position.entry_time.format("%Y-%m-%d %H:%M:%S UTC"));
                
                // Validate the recovered position
                let now = Utc::now();
                let position_age = now - position.entry_time;
                if position_age.num_hours() > 24 {
                    warn!("⚠️ Recovered position is {} hours old - may be stale", position_age.num_hours());
                }
                
                Ok(())
            }
            Ok(None) => {
                // Clear any existing position state to ensure consistency
                if self.current_position.is_some() {
                    info!("🔄 Clearing stale position state - no position found in database");
                    self.current_position = None;
                } else {
                    info!("💤 No open positions found in database");
                }
                Ok(())
            }
            Err(e) => {
                warn!("❌ Failed to recover positions: {}", e);
                // Don't clear existing position state on error - be conservative
                Err(e)
            }
        }
    }

    async fn close_position_in_database(&self, exit_price: f64) -> Result<()> {
        use urlencoding::encode;
        use serde_json::json;
        use std::time::Duration as StdDuration;
        
        // Try to use cached position ID first (most efficient)
        if let Some(position) = &self.current_position {
            if let Some(position_id) = &position.position_id {
                if !position_id.is_empty() {
                    info!("🆔 Using cached position ID: {}", position_id);
                    return self.close_position_with_id(position_id, exit_price).await;
                }
            }
        }
        
        // Fallback: Get position from database (less efficient but reliable)
        info!("🔍 No cached position ID, fetching from database...");
        return self.close_position_with_fallback(exit_price).await;
    }
    
    async fn close_position_with_id(&self, position_id: &str, exit_price: f64) -> Result<()> {
        use serde_json::json;
        
        // Quick health check before attempting to close
        if !self.check_database_health().await? {
            warn!("⚠️ Database health check failed, but proceeding with close attempt");
        }
        
        let close_request = json!({
            "position_id": position_id,
            "exit_price": exit_price,
            "transaction_hash": None::<String>,
            "fees": None::<f64>,
        });
        
        let close_url = format!("{}/positions/close", self.config.database_url);
        
        info!("🔗 Closing position with ID: {}", position_id);
        info!("📤 Close request: {}", serde_json::to_string_pretty(&close_request)?);
        
        // Retry logic with exponential backoff
        for attempt in 1..=3 {
            match self.attempt_close_position_request(&close_url, &close_request).await {
                Ok(_) => {
                    info!("✅ Successfully closed position {} in database at price: {}", position_id, exit_price);
                    return Ok(());
                }
                Err(e) => {
                    if attempt == 3 {
                        error!("❌ Failed to close position after 3 attempts: {}", e);
                        return Err(e);
                    }
                    let delay_ms = 100 * attempt; // 100ms, 200ms, 300ms
                    warn!("⚠️ Close attempt {} failed, retrying in {}ms: {}", attempt, delay_ms, e);
                    tokio::time::sleep(Duration::from_millis(delay_ms)).await;
                }
            }
        }
        
        Err(anyhow!("Failed to close position after all retry attempts"))
    }
    
    async fn close_position_with_fallback(&self, exit_price: f64) -> Result<()> {
        use urlencoding::encode;
        
        // First, get the open position from the database to get its ID
        let wallet_address = self.trading_executor.get_wallet_address()?;
        let encoded_pair = encode(&self.config.trading_pair);
        let url = format!("{}/positions/pair/{}/open", self.config.database_url, encoded_pair);
        
        info!("🔍 Attempting to close position in database:");
        info!("  📍 URL: {}", url);
        info!("  🎯 Trading pair: {}", self.config.trading_pair);
        info!("  🔗 Encoded pair: {}", encoded_pair);
        info!("  💰 Exit price: ${:.4}", exit_price);
        
        // Retry logic for GET request
        for attempt in 1..=3 {
            match self.attempt_get_position_request(&url).await {
                Ok(Some(position_id)) => {
                    info!("✅ Found position ID: {}", position_id);
                    return self.close_position_with_id(&position_id, exit_price).await;
                }
                Ok(None) => {
                    warn!("❌ No open position found to close");
                    return Ok(()); // No position to close is not an error
                }
                Err(e) => {
                    if attempt == 3 {
                        error!("❌ Failed to get position after 3 attempts: {}", e);
                        return Err(e);
                    }
                    let delay_ms = 100 * attempt;
                    warn!("⚠️ Get position attempt {} failed, retrying in {}ms: {}", attempt, delay_ms, e);
                    tokio::time::sleep(Duration::from_millis(delay_ms)).await;
                }
            }
        }
        
        Err(anyhow!("Failed to get position after all retry attempts"))
    }
    
    async fn attempt_close_position_request(&self, url: &str, request: &serde_json::Value) -> Result<()> {
        let start_time = std::time::Instant::now();
        
        let response = self.client.post(url)
            .timeout(Duration::from_secs(15))
            .json(request)
            .send()
            .await?;
            
        let duration = start_time.elapsed();
        info!("📡 Close response status: {} (took {:?})", response.status(), duration);
        
        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            let error_msg = format!("Failed to close position in database: {} - {}", status, text);
            
            // Log detailed error information for debugging
            error!("❌ Database close failed:");
            error!("  Status: {}", status);
            error!("  URL: {}", url);
            error!("  Request: {}", serde_json::to_string_pretty(request)?);
            error!("  Response: {}", text);
            error!("  Duration: {:?}", duration);
            
            return Err(anyhow!(error_msg));
        } else {
            let response_text = response.text().await.unwrap_or_default();
            info!("📊 Close response: {}", response_text);
            info!("✅ Database close successful in {:?}", duration);
            Ok(())
        }
    }
    
    async fn attempt_get_position_request(&self, url: &str) -> Result<Option<String>> {
        let start_time = std::time::Instant::now();
        
        let response = self.client.get(url)
            .timeout(Duration::from_secs(10))
            .send()
            .await?;
            
        let duration = start_time.elapsed();
        info!("📡 Get position response status: {} (took {:?})", response.status(), duration);
        
        if !response.status().is_success() {
            let error_msg = format!("Failed to get open position: {}", response.status());
            
            // Log detailed error information for debugging
            error!("❌ Database get position failed:");
            error!("  Status: {}", response.status());
            error!("  URL: {}", url);
            error!("  Duration: {:?}", duration);
            
            return Err(anyhow!(error_msg));
        }
        
        let api_response: serde_json::Value = response.json().await?;
        info!("📊 API Response: {}", serde_json::to_string_pretty(&api_response)?);
        
        // Check if data is null (no position found)
        if api_response["data"].is_null() {
            info!("💤 No open position found (data is null)");
            return Ok(None);
        }
        
        if let Some(position_data) = api_response["data"].as_object() {
            info!("✅ Found position data: {}", serde_json::to_string_pretty(position_data)?);
            
            if let Some(position_id) = position_data["id"].as_str() {
                info!("🆔 Position ID: {}", position_id);
                info!("✅ Database get position successful in {:?}", duration);
                return Ok(Some(position_id.to_string()));
            } else {
                return Err(anyhow!("No position ID found in response"));
            }
        } else {
            return Err(anyhow!("Invalid position data format in response"));
        }
    }

    // Helper functions for enhanced analysis
    fn get_recent_prices(&self, prices: &[PriceFeed], seconds_back: u64) -> Vec<PriceFeed> {
        let cutoff_time = Utc::now() - chrono::Duration::seconds(seconds_back as i64);
        prices.iter()
            .filter(|p| p.timestamp >= cutoff_time)
            .cloned()
            .collect()
    }

    fn analyze_market_regime(&self, prices: &[PriceFeed]) -> (String, f64) {
        if prices.len() < 50 {
            return ("Consolidating".to_string(), 0.0);
        }

        let price_values: Vec<f64> = prices.iter().map(|p| p.price).collect();
        
        // Calculate trend strength using linear regression
        let trend_strength = self.calculate_trend_strength(&price_values);
        
        // Calculate price range and volatility
        let min_price = price_values.iter().fold(f64::INFINITY, |a, &b| a.min(b));
        let max_price = price_values.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b));
        let price_range = (max_price - min_price) / min_price;
        let volatility = self.calculate_volatility(&price_values, 20).unwrap_or(0.02);
        
        // Determine market regime
        let regime = if trend_strength > 0.7 && price_range > 0.1 {
            "Trending"
        } else if volatility > 0.05 {
            "Volatile"
        } else if price_range < 0.05 {
            "Consolidating"
        } else {
            "Ranging"
        };
        
        (regime.to_string(), trend_strength)
    }

    fn calculate_trend_strength(&self, prices: &[f64]) -> f64 {
        if prices.len() < 20 {
            return 0.0;
        }
        
        let n = prices.len() as f64;
        let x_values: Vec<f64> = (0..prices.len()).map(|i| i as f64).collect();
        let y_values = prices.to_vec();
        
        let sum_x: f64 = x_values.iter().sum();
        let sum_y: f64 = y_values.iter().sum();
        let sum_xy: f64 = x_values.iter().zip(y_values.iter()).map(|(x, y)| x * y).sum();
        let sum_x2: f64 = x_values.iter().map(|x| x * x).sum();
        
        let slope = (n * sum_xy - sum_x * sum_y) / (n * sum_x2 - sum_x * sum_x);
        let avg_price = sum_y / n;
        
        // Normalize slope by average price to get percentage change
        let trend_strength = (slope / avg_price).abs();
        trend_strength.min(1.0) // Cap at 100%
    }

    fn calculate_support_resistance(&self, prices: &[PriceFeed]) -> (Option<f64>, Option<f64>) {
        if prices.len() < 20 {
            return (None, None);
        }
        
        let price_values: Vec<f64> = prices.iter().map(|p| p.price).collect();
        let current_price = price_values.last().unwrap();
        
        // Simple support/resistance calculation using recent highs and lows
        let recent_prices = &price_values[price_values.len().saturating_sub(20)..];
        let min_price = recent_prices.iter().fold(f64::INFINITY, |a, &b| a.min(b));
        let max_price = recent_prices.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b));
        
        let support = if min_price < *current_price { Some(min_price) } else { None };
        let resistance = if max_price > *current_price { Some(max_price) } else { None };
        
        (support, resistance)
    }

    fn calculate_volatility(&self, prices: &[f64], window: usize) -> Option<f64> {
        if prices.len() < window {
            return None;
        }
        
        let returns: Vec<f64> = prices.windows(2)
            .map(|w| (w[1] - w[0]) / w[0])
            .collect();
        
        let mean = returns.iter().sum::<f64>() / returns.len() as f64;
        let variance = returns.iter()
            .map(|r| (r - mean).powi(2))
            .sum::<f64>() / returns.len() as f64;
        
        Some(variance.sqrt())
    }

    fn calculate_dynamic_thresholds(&self, prices: &[PriceFeed]) -> crate::strategy::DynamicThresholds {
        // This is a simplified version - in practice, you'd want to use the strategy's method
        // For now, return reasonable defaults
        crate::strategy::DynamicThresholds {
            rsi_oversold: 30.0,
            rsi_overbought: 70.0,
            take_profit: 0.05, // 5%
            stop_loss: 0.03,    // 3%
            momentum_threshold: 0.02, // 2%
            volatility_multiplier: 1.5,
            market_regime: crate::strategy::MarketRegime::Ranging,
            trend_strength: 0.5,
            support_level: None,
            resistance_level: None,
        }
    }
    
    // Enhanced exit conditions for quick swaps
    async fn check_enhanced_exit_conditions(&self, signal: &TradingSignal, position: &Position, pnl: f64) -> Result<bool> {
        // Get recent price data for analysis
        let prices = self.fetch_price_history().await?;
        
        // 1. Traditional stop loss and take profit
        if pnl < -signal.stop_loss {
            info!("🛑 Stop loss triggered: {:.2}% < -{:.2}%", pnl * 100.0, signal.stop_loss * 100.0);
            return Ok(true);
        }
        
        if pnl > signal.take_profit {
            info!("💰 Take profit triggered: {:.2}% > {:.2}%", pnl * 100.0, signal.take_profit * 100.0);
            return Ok(true);
        }
        
        // 2. Momentum decay exit (if momentum is weakening while in profit)
        if pnl > 0.003 && self.strategy.detect_momentum_decay(&prices) {
            info!("📉 Momentum decay exit: Profit {:.2}% but momentum weakening", pnl * 100.0);
            return Ok(true);
        }
        
        // 3. RSI divergence exit (if RSI weakening while in profit)
        // Calculate current RSI and momentum from price data
        let price_values: Vec<f64> = prices.iter().map(|p| p.price).collect();
        if let Some(rsi) = self.strategy.calculate_rsi(&price_values, 14) {
            let price_momentum = self.strategy.calculate_price_momentum(&price_values).unwrap_or(0.0);
            if self.strategy.should_exit_rsi_divergence(rsi, price_momentum, pnl) {
                info!("📊 RSI divergence exit: RSI {:.1}, momentum {:.3}, profit {:.2}%", 
                      rsi, price_momentum, pnl * 100.0);
                return Ok(true);
            }
        }
        
        // 4. Time-based exit for small profits (if held too long with small profit)
        let duration = Utc::now() - position.entry_time;
        let max_hold_time_seconds = 1800; // 30 minutes max hold
        
        if duration.num_seconds() > max_hold_time_seconds && pnl > 0.002 {
            info!("⏰ Time-based exit: Held {}s with {:.2}% profit", 
                  duration.num_seconds(), pnl * 100.0);
            return Ok(true);
        }
        
        // 5. Small profit exit (if profit is small but stable)
        if pnl > 0.005 && pnl < 0.01 {
            // Check if price has been stable for last 5 minutes
            let recent_prices = self.get_recent_prices(&prices, 300); // 5 minutes
            if recent_prices.len() >= 5 {
                let price_volatility = self.calculate_volatility(
                    &recent_prices.iter().map(|p| p.price).collect::<Vec<f64>>(), 
                    5
                ).unwrap_or(0.0);
                
                if price_volatility < 0.005 { // Low volatility
                    info!("🎯 Small profit exit: {:.2}% profit with low volatility {:.3}%", 
                          pnl * 100.0, price_volatility * 100.0);
                    return Ok(true);
                }
            }
        }
        
        Ok(false)
    }

    async fn check_database_health(&self) -> Result<bool> {
        let health_url = format!("{}/health", self.config.database_url);
        
        match self.client.get(&health_url)
            .timeout(Duration::from_secs(5))
            .send()
            .await {
            Ok(response) => {
                if response.status().is_success() {
                    debug!("✅ Database health check passed");
                    Ok(true)
                } else {
                    warn!("⚠️ Database health check failed: {}", response.status());
                    Ok(false)
                }
            }
            Err(e) => {
                warn!("⚠️ Database health check error: {}", e);
                Ok(false)
            }
        }
    }
} 