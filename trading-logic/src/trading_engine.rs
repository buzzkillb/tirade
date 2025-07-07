use crate::config::Config;
use crate::models::{PriceFeed, TechnicalIndicators, TradingSignal, SignalType};
use crate::strategy::TradingStrategy;
use anyhow::{Result, anyhow};
use reqwest::Client;
use std::time::Duration;

use tracing::{info, warn, error, debug};
use chrono::Utc;

pub struct TradingEngine {
    config: Config,
    strategy: TradingStrategy,
    client: Client,
    current_position: Option<Position>,
    last_analysis_time: Option<chrono::DateTime<Utc>>,
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

        Ok(Self {
            config,
            strategy,
            client,
            current_position: None,
            last_analysis_time: None,
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
        info!("═══════════════════════════════════════════════════════════════");
        info!("");

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

        // Step 4: Analyze and generate signal
        let signal = self.strategy.analyze(&prices, &indicators);

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

        let response = self.client.get(&url).send().await?;
        let api_response: crate::models::ApiResponse<Vec<PriceFeed>> = response.json().await?;

        if api_response.success {
            Ok(api_response.data.len())
        } else {
            Ok(0)
        }
    }

    async fn fetch_price_history(&self) -> Result<Vec<PriceFeed>> {
        let url = format!(
            "{}/prices/{}/history?hours=24",
            self.config.database_url,
            urlencoding::encode(&self.config.trading_pair)
        );

        let response = self.client.get(&url).send().await?;
        let api_response: crate::models::ApiResponse<Vec<PriceFeed>> = response.json().await?;

        if api_response.success {
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
                    // Open long position
                    self.open_position(signal.price, PositionType::Long).await?;
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
                } else {
                    debug!("Already in position, ignoring BUY signal");
                }
            }
            SignalType::Sell => {
                if let Some(position) = &self.current_position {
                    // Extract position data before mutable borrow
                    let entry_price = position.entry_price;
                    let entry_time = position.entry_time;
                    let position_type = position.position_type.clone();
                    
                    // Close position
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
                        let duration = Utc::now() - entry_time;
                        self.close_position(signal.price).await?;
                        
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
                        let duration = Utc::now() - entry_time;
                        self.close_position(signal.price).await?;
                        
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
} 