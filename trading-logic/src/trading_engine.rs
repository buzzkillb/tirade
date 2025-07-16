use crate::config::Config;
use crate::models::{PriceFeed, TechnicalIndicators, TradingSignal, SignalType};
use crate::strategy::TradingStrategy;
use crate::trading_executor::TradingExecutor;
use crate::ml_strategy::MLStrategy;
use crate::database_service::DatabaseService;
use crate::position_manager::{PositionManager, WalletStats};
use crate::signal_processor::SignalProcessor;
use anyhow::Result;
use reqwest::Client;
use std::time::Duration;
use tracing::{info, warn, error, debug};
use chrono::Utc;
use serde_json;

pub struct TradingEngine {
    config: Config,
    strategy: TradingStrategy,
    client: Client,
    trading_executors: Vec<TradingExecutor>,
    ml_strategy: MLStrategy,
    database_service: DatabaseService,
    position_manager: PositionManager,
    signal_processor: SignalProcessor,
    last_analysis_time: Option<chrono::DateTime<chrono::Utc>>,
}

impl TradingEngine {
    pub async fn new(config: Config) -> Result<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(10))
            .build()?;

        let strategy = TradingStrategy::new(config.clone());
        
        // Create trading executors for each wallet
        let mut trading_executors = Vec::new();
        let wallet_count = config.get_wallet_count();
        
        for i in 0..wallet_count {
            let wallet_key = config.get_wallet_key(i).unwrap().clone();
            let wallet_name = config.get_wallet_name(i).unwrap().clone();
            
            let executor = TradingExecutor::new_with_wallet(i, wallet_name, Some(wallet_key))?;
            trading_executors.push(executor);
        }

        // Initialize services
        let database_service = DatabaseService::new(config.database_url.clone());
        let position_manager = PositionManager::new(wallet_count);
        let signal_processor = SignalProcessor::new(config.trading_pair.clone());
        let ml_strategy = MLStrategy::new(config.clone());

        info!("🏦 Initialized {} wallets for multiwallet trading", wallet_count);
        for (i, executor) in trading_executors.iter().enumerate() {
            info!("  Wallet {}: {}", i + 1, executor.get_wallet_name());
        }

        Ok(Self {
            config,
            strategy,
            client,
            trading_executors,
            ml_strategy,
            database_service,
            position_manager,
            signal_processor,
            last_analysis_time: None,
        })
    }

    pub async fn run(&mut self) -> Result<()> {
        info!("🚀 Starting Trading Logic Engine...");
        self.log_startup_info();

        // Initialize ML strategy
        if let Err(e) = self.ml_strategy.load_trade_history(&self.config.trading_pair).await {
            warn!("Failed to load ML trade history: {}", e);
        }

        // Post initial trading config
        if let Err(e) = self.post_initial_config().await {
            warn!("Failed to post initial trading config: {}", e);
        }

        // Recover positions
        if let Err(e) = self.recover_positions().await {
            warn!("Failed to recover positions: {}", e);
        }

        // Main trading loop
        loop {
            let start_time = Utc::now();
            
            if let Err(e) = self.trading_cycle().await {
                error!("Trading cycle error: {}", e);
            }
            
            let duration = Utc::now() - start_time;
            debug!("✅ Trading cycle completed in {}ms", duration.num_milliseconds());
            
            // Sleep for 30 seconds between cycles
            tokio::time::sleep(Duration::from_secs(30)).await;
        }
    }

    async fn trading_cycle(&mut self) -> Result<()> {
        // Step 1: Fetch price data
        let prices = self.fetch_price_history().await?;
        if prices.is_empty() {
            warn!("⚠️ No price data available");
            return Ok(());
        }

        // Step 2: Calculate technical indicators
        let strategy_indicators = self.strategy.calculate_custom_indicators(&prices);
        let consolidated_indicators = self.calculate_consolidated_indicators(&prices, &strategy_indicators);

        // Step 3: Post indicators to database
        if let Err(e) = self.database_service.post_consolidated_indicators(&consolidated_indicators, &self.config.trading_pair).await {
            warn!("Failed to post consolidated indicators: {}", e);
        }

        // Step 4: Generate and enhance signal
        let signal = self.strategy.analyze(&prices, &consolidated_indicators);
        let enhanced_signal = self.enhance_signal_with_ml(signal, &prices, &strategy_indicators)?;

        // Step 5: Post signal to database
        if let Err(e) = self.database_service.post_trading_signal(&enhanced_signal, &self.config.trading_pair).await {
            warn!("Failed to post signal: {}", e);
        }

        // Step 6: Process signal
        self.process_signal(&enhanced_signal, &prices, &strategy_indicators).await?;

        // Step 7: Log analysis
        self.log_analysis(&enhanced_signal, &prices);

        Ok(())
    }

    async fn process_signal(&mut self, signal: &TradingSignal, prices: &[PriceFeed], indicators: &crate::models::TradingIndicators) -> Result<()> {
        match signal.signal_type {
            SignalType::Buy => {
                self.signal_processor.handle_buy_signal(
                    signal,
                    &self.trading_executors,
                    &mut self.position_manager,
                    &self.database_service,
                ).await?;
            }
            SignalType::Sell => {
                self.signal_processor.handle_sell_signal(
                    signal,
                    &self.trading_executors,
                    &mut self.position_manager,
                    &self.database_service,
                    &mut self.ml_strategy,
                ).await?;
            }
            SignalType::Hold => {
                // Check for exit conditions on existing positions
                self.signal_processor.check_exit_conditions(
                    prices,
                    indicators,
                    &self.trading_executors,
                    &mut self.position_manager,
                    &self.database_service,
                    &mut self.ml_strategy,
                    &self.strategy,
                ).await?;
            }
        }
        Ok(())
    }

    fn enhance_signal_with_ml(&mut self, signal: TradingSignal, prices: &[PriceFeed], indicators: &crate::models::TradingIndicators) -> Result<TradingSignal> {
        match self.ml_strategy.enhance_signal(&signal, prices, indicators) {
            Ok(enhanced) => {
                if enhanced.signal_type != signal.signal_type || (enhanced.confidence - signal.confidence).abs() > 0.1 {
                    info!("🤖 ML enhanced: {:?} ({}%) → {:?} ({}%)", 
                          signal.signal_type, (signal.confidence * 100.0) as i32,
                          enhanced.signal_type, (enhanced.confidence * 100.0) as i32);
                }
                Ok(enhanced)
            }
            Err(e) => {
                warn!("⚠️ ML enhancement failed: {} - using original signal", e);
                Ok(signal)
            }
        }
    }

    async fn fetch_price_history(&self) -> Result<Vec<PriceFeed>> {
        use urlencoding::encode;
        
        // Try to fetch 1-minute candles first
        let candle_url = format!("{}/candles/{}/1m?limit=200", 
                                self.config.database_url, 
                                encode(&self.config.trading_pair));
        
        let response = self.client.get(&candle_url).send().await?;
        let text = response.text().await?;
        
        if !text.trim().is_empty() {
            let api_response: Result<crate::models::ApiResponse<Vec<crate::models::Candle>>, _> = serde_json::from_str(&text);
            if let Ok(crate::models::ApiResponse { success: true, data: Some(candles), .. }) = api_response {
                if !candles.is_empty() {
                    info!("📊 Using {} 1-minute candles for analysis", candles.len());
                    
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
        }

        // Fallback to raw price data
        let url = format!("{}/prices/{}", self.config.database_url, encode(&self.config.trading_pair));
        let response = self.client.get(&url).send().await?;
        let text = response.text().await?;
        
        if text.trim().is_empty() {
            return Ok(vec![]);
        }
        
        let api_response: Result<crate::models::ApiResponse<Vec<PriceFeed>>, _> = serde_json::from_str(&text);
        match api_response {
            Ok(crate::models::ApiResponse { success: true, data: Some(prices), .. }) => {
                info!("📊 Using {} raw price records for analysis", prices.len());
                Ok(prices)
            }
            _ => Ok(vec![]),
        }
    }

    fn calculate_consolidated_indicators(&self, prices: &[PriceFeed], strategy_indicators: &crate::models::TradingIndicators) -> TechnicalIndicators {
        let current_price = prices.first().map(|p| p.price).unwrap_or(0.0);
        let now = Utc::now();
        
        // Calculate RSI14
        let rsi_14 = if prices.len() >= 14 {
            let price_values: Vec<f64> = prices.iter().map(|p| p.price).collect();
            self.calculate_rsi_14(&price_values)
        } else {
            None
        };
        
        let sma_20 = strategy_indicators.sma_short;
        let sma_50 = strategy_indicators.sma_long;
        
        // Calculate SMA200
        let sma_200 = if prices.len() >= 200 {
            let price_values: Vec<f64> = prices.iter().map(|p| p.price).collect();
            self.calculate_sma_200(&price_values)
        } else {
            None
        };
        
        // Calculate 24h price change
        let (price_change_24h, price_change_percent_24h) = if prices.len() >= 24 * 60 {
            let current = prices[prices.len() - 1].price;
            let past_24h = prices[prices.len() - 24 * 60].price;
            let change = current - past_24h;
            let change_percent = (change / past_24h) * 100.0;
            (Some(change), Some(change_percent))
        } else {
            (None, None)
        };
        
        // Calculate 24h volatility
        let volatility_24h = if prices.len() >= 24 * 60 {
            let price_values: Vec<f64> = prices.iter().map(|p| p.price).collect();
            self.calculate_volatility_24h(&price_values)
        } else {
            None
        };
        
        TechnicalIndicators {
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
        if prices.len() < 24 * 60 {
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

    async fn recover_positions(&mut self) -> Result<()> {
        let wallet_addresses: Vec<String> = self.trading_executors
            .iter()
            .map(|executor| executor.get_wallet_address())
            .collect::<Result<Vec<_>, _>>()?;
            
        self.position_manager.recover_positions(&self.database_service, &wallet_addresses).await
    }

    async fn post_initial_config(&self) -> Result<()> {
        if self.trading_executors.is_empty() {
            return Ok(());
        }

        self.database_service.post_trading_config(
            &self.config.trading_pair,
            self.config.min_confidence_threshold,
            self.trading_executors[0].get_position_size_percentage(),
            self.trading_executors[0].get_slippage_tolerance(),
        ).await
    }

    fn log_startup_info(&self) {
        info!("  📊 Trading Pair: {}", self.config.trading_pair);
        info!("  🏦 Wallets: {}", self.trading_executors.len());
        
        for executor in &self.trading_executors {
            info!("  🔄 {}: Trading Execution: {}", 
                  executor.get_wallet_name(),
                  if executor.is_trading_enabled() { "ENABLED" } else { "PAPER TRADING" });
            info!("  💰 {}: Position Size: {:.1}% of balance", 
                  executor.get_wallet_name(),
                  executor.get_position_size_percentage() * 100.0);
        }
        
        if !self.trading_executors.is_empty() {
            info!("  📊 Slippage Tolerance: {:.1}%", self.trading_executors[0].get_slippage_tolerance() * 100.0);
            info!("  🎯 Min Confidence: {:.1}%", self.trading_executors[0].get_min_confidence_threshold() * 100.0);
        }
    }

    fn log_analysis(&self, signal: &TradingSignal, prices: &[PriceFeed]) {
        if prices.is_empty() || signal.signal_type == SignalType::Hold {
            return;
        }

        let current_price = prices.last().unwrap().price;
        let price_change = if prices.len() >= 2 {
            let current = prices[prices.len() - 1].price;
            let previous = prices[prices.len() - 2].price;
            (current - previous) / previous
        } else {
            0.0
        };

        info!("📊 Analysis: {:?} at ${:.4} | Change: {:.2}% | Conf: {:.0}%", 
              signal.signal_type, current_price, price_change * 100.0, signal.confidence * 100.0);
    }

    // Helper method to get wallet statistics
    pub fn get_wallet_stats(&self) -> Vec<WalletStats> {
        self.position_manager.get_wallet_stats(&self.config.wallet_names)
    }
}