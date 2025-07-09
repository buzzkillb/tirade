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
    entry_price: f64,
    entry_time: chrono::DateTime<Utc>,
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
        // Step 0: Retry position recovery if we don't have a position
        if self.current_position.is_none() {
            debug!("🔄 No current position detected, attempting recovery...");
            if let Err(e) = self.recover_positions().await {
                debug!("⚠️  Position recovery failed: {}", e);
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

        // Step 3: Fetch technical indicators
        let indicators = self.fetch_technical_indicators().await?;

        // Step 3.5: Post all technical indicators to database
        if let Err(e) = self.post_all_indicators(&indicators).await {
            warn!("Failed to post indicators: {}", e);
        }

        // Step 4: Analyze and generate signal
        let signal = self.strategy.analyze(&prices, &indicators);

        // Step 4.5: Post trading signal to database
        if let Err(e) = self.post_trading_signal(&signal).await {
            warn!("Failed to post signal: {}", e);
        }

        // Step 5: Execute trading logic
        self.execute_signal(&signal).await?;

        // Step 6: Log the analysis
        self.log_analysis(&signal, &prices, &indicators);

        Ok(())
    }

    async fn get_data_point_count(&self) -> Result<usize> {
        let url = format!(
            "{}/prices/{}/history?hours=24",
            self.config.database_url,
            urlencoding::encode(&self.config.trading_pair)
        );

        info!("🔍 Fetching data point count from: {}", url);
        
        let response = self.client.get(&url).send().await?;
        let api_response: crate::models::ApiResponse<Vec<PriceFeed>> = response.json().await?;

        info!("📊 API Response - Success: {}, Data points: {}", api_response.success, api_response.data.len());

        if api_response.success {
            Ok(api_response.data.len())
        } else {
            warn!("⚠️ API call failed: {:?}", api_response.message);
            Ok(0)
        }
    }

    async fn fetch_price_history(&self) -> Result<Vec<PriceFeed>> {
        // Fetch 30 days of data for multi-timeframe analysis
        let url = format!(
            "{}/prices/{}/history?hours=720", // 30 days = 720 hours
            self.config.database_url,
            urlencoding::encode(&self.config.trading_pair)
        );

        let response = self.client.get(&url).send().await?;
        let api_response: crate::models::ApiResponse<Vec<PriceFeed>> = response.json().await?;

        if api_response.success {
            info!("📊 Fetched {} price records for multi-timeframe analysis", api_response.data.len());
            Ok(api_response.data)
        } else {
            Err(anyhow!("Failed to fetch price history: {:?}", api_response.message))
        }
    }

    async fn fetch_technical_indicators(&self) -> Result<TechnicalIndicators> {
        let url = format!(
            "{}/indicators/{}?hours=24",
            self.config.database_url,
            urlencoding::encode(&self.config.trading_pair)
        );

        let response = self.client.get(&url).send().await?;
        let api_response: crate::models::ApiResponse<TechnicalIndicators> = response.json().await?;

        if api_response.success {
            Ok(api_response.data)
        } else {
            Err(anyhow!("Failed to fetch technical indicators: {:?}", api_response.message))
        }
    }

    async fn execute_signal(&mut self, signal: &TradingSignal) -> Result<()> {
        match signal.signal_type {
            SignalType::Buy => {
                if self.current_position.is_none() {
                    info!("🟢 BUY signal detected - no current position, executing trade...");
                    // Execute the trade using trading executor
                    match self.trading_executor.execute_signal(signal).await {
                        Ok(true) => {
                            // Trade executed successfully (or paper trading)
                            self.open_position(signal.price, PositionType::Long).await?;
                            
                            // Post position to database
                            if let Some(position) = &self.current_position {
                                if let Err(e) = self.post_position(position, signal.take_profit, signal.stop_loss).await {
                                    warn!("Failed to post position: {}", e);
                                }
                            }
                            
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
                        Ok(false) => {
                            warn!("⚠️  BUY signal execution failed or was skipped");
                        }
                        Err(e) => {
                            error!("❌ BUY signal execution error: {}", e);
                        }
                    }
                } else {
                    info!("⚠️  BUY signal detected but already in position - ignoring signal");
                    info!("📊 Current position: {:?} at ${:.4}", 
                          self.current_position.as_ref().unwrap().position_type,
                          self.current_position.as_ref().unwrap().entry_price);
                }
            }
            SignalType::Sell => {
                if let Some(position) = &self.current_position {
                    // Extract position data before mutable borrow
                    let entry_price = position.entry_price;
                    let entry_time = position.entry_time;
                    let position_type = position.position_type.clone();
                    
                    // Execute the trade using trading executor
                    match self.trading_executor.execute_signal(signal).await {
                        Ok(true) => {
                            // Trade executed successfully (or paper trading)
                            let pnl = self.calculate_pnl(signal.price, position);
                            let duration = Utc::now() - entry_time;
                            self.close_position(signal.price).await?;
                            
                            // Post trade to database
                            if let Err(e) = self.post_trade(entry_price, signal.price, entry_time, &position_type, pnl).await {
                                warn!("Failed to post trade: {}", e);
                            }
                            
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
                        Ok(false) => {
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
                    
                    // Stop loss check using dynamic threshold
                    if pnl < -signal.stop_loss {
                        let entry_price = position.entry_price;
                        let entry_time = position.entry_time;
                        let position_type = position.position_type.clone();
                        let duration = Utc::now() - entry_time;
                        self.close_position(signal.price).await?;
                        
                        // Post trade to database
                        if let Err(e) = self.post_trade(entry_price, signal.price, entry_time, &position_type, pnl).await {
                            warn!("Failed to post trade: {}", e);
                        }
                        
                        info!("");
                        info!("🛑 DYNAMIC STOP LOSS TRIGGERED");
                        info!("═══════════════════════════════════════════════════════════════");
                        info!("  💰 Exit Price: ${:.4}", signal.price);
                        info!("  📈 Entry Price: ${:.4}", entry_price);
                        info!("  💸 Loss: {:.2}%", pnl * 100.0);
                        info!("  🎯 Dynamic Stop Loss Threshold: {:.2}%", signal.stop_loss * 100.0);
                        info!("  ⏱️  Duration: {}s", duration.num_seconds());
                        info!("  ⏰ Timestamp: {}", Utc::now().format("%Y-%m-%d %H:%M:%S UTC"));
                        info!("═══════════════════════════════════════════════════════════════");
                        info!("");
                    }
                    // Take profit check using dynamic threshold
                    else if pnl > signal.take_profit {
                        let entry_price = position.entry_price;
                        let entry_time = position.entry_time;
                        let position_type = position.position_type.clone();
                        let duration = Utc::now() - entry_time;
                        self.close_position(signal.price).await?;
                        
                        // Post trade to database
                        if let Err(e) = self.post_trade(entry_price, signal.price, entry_time, &position_type, pnl).await {
                            warn!("Failed to post trade: {}", e);
                        }
                        
                        info!("");
                        info!("💰 DYNAMIC TAKE PROFIT TRIGGERED");
                        info!("═══════════════════════════════════════════════════════════════");
                        info!("  💰 Exit Price: ${:.4}", signal.price);
                        info!("  📈 Entry Price: ${:.4}", entry_price);
                        info!("  💰 Profit: {:.2}%", pnl * 100.0);
                        info!("  🎯 Dynamic Take Profit Threshold: {:.2}%", signal.take_profit * 100.0);
                        info!("  ⏱️  Duration: {}s", duration.num_seconds());
                        info!("  ⏰ Timestamp: {}", Utc::now().format("%Y-%m-%d %H:%M:%S UTC"));
                        info!("═══════════════════════════════════════════════════════════════");
                        info!("");
                    }
                }
            }
        }

        Ok(())
    }

    async fn open_position(&mut self, price: f64, position_type: PositionType) -> Result<()> {
        let log_type = position_type.clone();
        self.current_position = Some(Position {
            entry_price: price,
            entry_time: Utc::now(),
            position_type,
        });
        
        info!("📈 Opened {:?} position at ${:.4}", log_type, price);
        Ok(())
    }

    async fn close_position(&mut self, price: f64) -> Result<()> {
        if let Some(position) = &self.current_position {
            let pnl = self.calculate_pnl(price, position);
            let duration = Utc::now() - position.entry_time;
            
            info!("📉 Closed position at ${:.4} - PnL: {:.2}% (Duration: {}s)", 
                  price, pnl * 100.0, duration.num_seconds());
        }
        
        self.current_position = None;
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
        info!("🎯 Trading Analysis Report");
        info!("═══════════════════════════════════════════════════════════════");
        info!("  💰 Current Price: ${:.4}", current_price);
        
        if let Some(change) = price_change {
            let change_emoji = if change > 0.0 { "📈" } else if change < 0.0 { "📉" } else { "➡️" };
            info!("  {} Price Change: {:.3}%", change_emoji, change);
        }
        
        info!("  📊 Data Points: {} | Signal: {:?}", price_count, signal.signal_type);
        info!("  🎯 Confidence: {:.1}%", signal.confidence * 100.0);
        
        // Position status
        if let Some(position) = &self.current_position {
            let pnl = self.calculate_pnl(current_price, position);
            let duration = Utc::now() - position.entry_time;
            let pnl_emoji = if pnl > 0.0 { "🟢" } else if pnl < 0.0 { "🔴" } else { "🟡" };
            
            info!("  📈 Active Position: {:?} | Entry: ${:.4}", position.position_type, position.entry_price);
            info!("  {} Unrealized PnL: {:.2}% | Duration: {}s", pnl_emoji, pnl * 100.0, duration.num_seconds());
        } else {
            info!("  💤 No active position");
        }
        
        // Technical indicators
        info!("");
        info!("📈 Technical Indicators:");
        if let Some(rsi) = indicators.rsi_14 {
            let rsi_status = if rsi > 70.0 { "🔴 Overbought" } else if rsi < 30.0 { "🟢 Oversold" } else { "🟡 Neutral" };
            info!("  📊 RSI (14): {:.2} {}", rsi, rsi_status);
        }
        if let Some(sma_20) = indicators.sma_20 {
            let sma_status = if current_price > sma_20 { "📈 Above" } else { "📉 Below" };
            info!("  📈 SMA (20): {:.4} {}", sma_20, sma_status);
        }
        if let Some(volatility) = indicators.volatility_24h {
            let vol_status = if volatility > 0.05 { "🔥 High" } else if volatility > 0.02 { "⚡ Medium" } else { "❄️ Low" };
            info!("  📊 Volatility (24h): {:.2}% {}", volatility * 100.0, vol_status);
        }
        if let Some(price_change_24h) = indicators.price_change_24h {
            let change_emoji = if price_change_24h > 0.0 { "📈" } else if price_change_24h < 0.0 { "📉" } else { "➡️" };
            info!("  {} 24h Change: {:.2}%", change_emoji, price_change_24h * 100.0);
        }
        
        // Signal reasoning
        info!("");
        info!("🧠 Signal Analysis:");
        if signal.reasoning.is_empty() {
            info!("  💭 No specific reasoning available");
        } else {
            for (i, reason) in signal.reasoning.iter().enumerate() {
                info!("  {}. {}", i + 1, reason);
            }
        }
        
        // Market sentiment based on indicators
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
        
        let sentiment = if bullish_signals > bearish_signals { "🐂 Bullish" } 
                       else if bearish_signals > bullish_signals { "🐻 Bearish" } 
                       else { "🦀 Sideways" };
        
        info!("  🎭 Market Sentiment: {} ({} bullish, {} bearish signals)", 
              sentiment, bullish_signals, bearish_signals);
        
        info!("═══════════════════════════════════════════════════════════════");
        info!("");
    }

    // Database API methods for posting data
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

    async fn post_position(&self, position: &Position, take_profit: f64, stop_loss: f64) -> Result<()> {
        // Get wallet address from Solana private key
        let wallet_address = self.trading_executor.get_wallet_address()?;

        // Ensure wallet exists in the database
        let create_wallet_request = serde_json::json!({
            "address": wallet_address,
        });
        let wallet_url = format!("{}/wallets", self.config.database_url);
        let wallet_response = self.client.post(&wallet_url)
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
            "quantity": 1.0, // Default quantity since Position struct doesn't have quantity field
        });

        let url = format!("{}/positions", self.config.database_url);
        
        let response = self.client.post(&url)
            .json(&create_position_request)
            .send()
            .await?;
            
        if !response.status().is_success() {
            warn!("Failed to post position: {}", response.status());
        } else {
            debug!("Posted position: {:?} at ${:.4}", position.position_type, position.entry_price);
        }
        
        Ok(())
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
        let url = format!("{}/positions/pair/{}/open", self.config.database_url, 
                         urlencoding::encode(&self.config.trading_pair));
        
        let response = self.client.get(&url).send().await?;
        
        if response.status().is_success() {
            let api_response: crate::models::ApiResponse<Option<PositionDb>> = response.json().await?;
            
            if api_response.success {
                if let Some(position_db) = api_response.data {
                    let position = Position {
                        entry_price: position_db.entry_price,
                        entry_time: position_db.entry_time,
                        position_type: match position_db.position_type.as_str() {
                            "long" => PositionType::Long,
                            "short" => PositionType::Short,
                            _ => return Err(anyhow!("Invalid position type: {}", position_db.position_type)),
                        },
                    };
                    Ok(Some(position))
                } else {
                    Ok(None)
                }
            } else {
                warn!("Failed to fetch open positions: {:?}", api_response.message);
                Ok(None)
            }
        } else {
            warn!("Failed to fetch open positions: {}", response.status());
            Ok(None)
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
        
        if let Some(position) = self.fetch_open_positions().await? {
            self.current_position = Some(position.clone());
            info!("📈 Recovered {} position: Entry ${:.4} at {}", 
                  match position.position_type {
                      PositionType::Long => "Long",
                      PositionType::Short => "Short",
                  },
                  position.entry_price,
                  position.entry_time.format("%Y-%m-%d %H:%M:%S UTC"));
        } else {
            info!("💤 No open positions found in database");
        }
        
        Ok(())
    }
} 