use crate::config::Config;
use crate::models::{PriceFeed, TechnicalIndicators, TradingSignal, SignalType, TechnicalIndicator, TradingSignalDb, PositionDb, TradeDb, TradingConfigDb};
use crate::strategy::TradingStrategy;
use crate::trading_executor::TradingExecutor;
use crate::ml_strategy::{MLStrategy, TradeResult};
use anyhow::{Result, anyhow};
use reqwest::Client;
use std::time::Duration;

use tracing::{info, warn, error, debug};
use chrono::{Utc, DateTime};

pub struct TradingEngine {
    config: Config,
    strategy: TradingStrategy,
    client: Client,
    wallet_positions: Vec<Option<Position>>, // One position per wallet
    last_analysis_time: Option<chrono::DateTime<Utc>>,
    trading_executors: Vec<TradingExecutor>, // One executor per wallet
    ml_strategy: MLStrategy,
}

#[derive(Debug, Clone)]
pub struct Position {
    pub position_id: Option<String>, // Store the database position ID to avoid GET request
    pub entry_price: f64,
    pub entry_time: chrono::DateTime<Utc>,
    pub quantity: f64,
    pub position_type: PositionType,
}

#[derive(Debug, Clone)]
pub enum PositionType {
    Long,
    Short,
}

#[derive(Debug)]
pub struct WalletStats {
    pub wallet_index: usize,
    pub wallet_name: String,
    pub has_position: bool,
    pub position_entry_price: Option<f64>,
    pub position_age_hours: Option<i64>,
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

        // Initialize position tracking for each wallet
        let wallet_positions = vec![None; wallet_count];

        info!("🏦 Initialized {} wallets for multiwallet trading", wallet_count);
        for (i, executor) in trading_executors.iter().enumerate() {
            info!("  Wallet {}: {}", i + 1, executor.get_wallet_name());
        }

        Ok(Self {
            config: config.clone(),
            strategy,
            client,
            wallet_positions,
            last_analysis_time: None,
            trading_executors,
            ml_strategy: MLStrategy::new(config),
        })
    }

    pub async fn run(&mut self) -> Result<()> {
        info!("🚀 Starting Trading Logic Engine...");
        info!("  📊 Trading Pair: {}", self.config.trading_pair);
        info!("  🏦 Wallets: {}", self.trading_executors.len());
        
        // Log info for each wallet
        for (i, executor) in self.trading_executors.iter().enumerate() {
            info!("  🔄 {}: Trading Execution: {}", 
                  executor.get_wallet_name(),
                  if executor.is_trading_enabled() { "ENABLED" } else { "PAPER TRADING" });
            info!("  💰 {}: Position Size: {:.1}% of balance", 
                  executor.get_wallet_name(),
                  executor.get_position_size_percentage() * 100.0);
        }
        
        info!("  📊 Slippage Tolerance: {:.1}%", self.trading_executors[0].get_slippage_tolerance() * 100.0);
        info!("  🎯 Min Confidence: {:.1}%", self.trading_executors[0].get_min_confidence_threshold() * 100.0);

        // Load ML trade history from database
        if let Err(e) = self.ml_strategy.load_trade_history(&self.config.trading_pair).await {
            warn!("Failed to load ML trade history: {}", e);
        }

        // Post initial trading config to database
        if let Err(e) = self.post_trading_config_multiwallet().await {
            warn!("Failed to post initial trading config: {}", e);
        }

        // Recover any existing positions from database
        if let Err(e) = self.recover_positions().await {
            warn!("Failed to recover positions: {}", e);
        }

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

    fn log_analysis(&self, signal: &TradingSignal, prices: &[PriceFeed], _indicators: &TechnicalIndicators) {
        if prices.is_empty() {
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

        // Only log analysis for actual signals, not holds
        if signal.signal_type != SignalType::Hold {
            info!("📊 Analysis: {:?} at ${:.4} | Change: {:.2}% | Conf: {:.0}%", 
                  signal.signal_type, current_price, price_change * 100.0, signal.confidence * 100.0);
        }
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

    async fn post_position(&self, position: &Position) -> Result<String> {
        // This method is deprecated - use post_position_for_wallet instead
        return Err(anyhow!("post_position is deprecated - use post_position_for_wallet"));
    }

    async fn post_position_for_wallet(&self, position: &Position, wallet_index: usize) -> Result<String> {
        // Get wallet address from the specific wallet executor
        let executor = &self.trading_executors[wallet_index];
        let wallet_address = executor.get_wallet_address()?;

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
        // This method is deprecated - use post_trading_config_multiwallet instead
        return Err(anyhow!("post_trading_config is deprecated - use post_trading_config_multiwallet"));
    }

    async fn post_trading_config_multiwallet(&self) -> Result<()> {
        if self.trading_executors.is_empty() {
            return Err(anyhow!("No trading executors available"));
        }

        let create_config_request = serde_json::json!({
            "name": format!("{}_config", self.config.trading_pair),
            "pair": self.config.trading_pair,
            "min_confidence_threshold": self.config.min_confidence_threshold * 100.0,
            "position_size_percent": self.trading_executors[0].get_position_size_percentage() * 100.0,
            "slippage_tolerance_percent": self.trading_executors[0].get_slippage_tolerance() * 100.0,
        });

        let url = format!("{}/configs", self.config.database_url);
        
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
        info!("🔄 Recovering positions from database for {} wallets...", self.trading_executors.len());
        
        for (wallet_index, executor) in self.trading_executors.iter().enumerate() {
            match self.fetch_open_positions_for_wallet(wallet_index).await {
                Ok(Some(position)) => {
                    self.wallet_positions[wallet_index] = Some(position.clone());
                    info!("📈 {} recovered position: Entry ${:.4}", 
                          executor.get_wallet_name(), position.entry_price);
                }
                Ok(None) => {
                    info!("💤 {} no open positions", executor.get_wallet_name());
                }
                Err(e) => {
                    warn!("❌ {} failed to recover positions: {}", executor.get_wallet_name(), e);
                }
            }
        }
        
        Ok(())
    }

    async fn fetch_open_positions_for_wallet(&self, wallet_index: usize) -> Result<Option<Position>> {
        let executor = &self.trading_executors[wallet_index];
        let wallet_address = executor.get_wallet_address()?;
        
        let url = format!("{}/positions/wallet/{}/open", self.config.database_url, 
                         urlencoding::encode(&wallet_address));
        
        let response = self.client.get(&url).send().await?;
        let text = response.text().await?;
        
        if text.trim().is_empty() {
            return Ok(None);
        }
        
        let api_response: Result<crate::models::ApiResponse<Option<PositionDb>>, _> = serde_json::from_str(&text);
        match api_response {
            Ok(crate::models::ApiResponse { success: true, data: Some(Some(position_db)), .. }) => {
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
            _ => Ok(None),
        }
    }

    async fn close_position_in_database(&self, exit_price: f64) -> Result<()> {
        // This method is deprecated - use close_position_in_database_for_wallet instead
        return Err(anyhow!("close_position_in_database is deprecated - use close_position_in_database_for_wallet"));
    }

    async fn close_position_in_database_for_wallet(&self, wallet_index: usize, exit_price: f64) -> Result<()> {
        if let Some(position) = &self.wallet_positions[wallet_index] {
            if let Some(position_id) = &position.position_id {
                let close_request = serde_json::json!({
                    "position_id": position_id,
                    "exit_price": exit_price,
                    "transaction_hash": None::<String>,
                    "fees": None::<f64>,
                });

                let close_url = format!("{}/positions/close", self.config.database_url);
                let response = self.client.post(&close_url)
                    .json(&close_request)
                    .send()
                    .await?;

                if !response.status().is_success() {
                    return Err(anyhow!("Failed to close position: {}", response.status()));
                }

                let executor = &self.trading_executors[wallet_index];
                info!("✅ {} position closed in database", executor.get_wallet_name());
            }
        }
        Ok(())
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
    }mpting to close
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
    
    async fn close_position_with_fallback(&self, _exit_price: f64) -> Result<()> {
        // This method is deprecated - use multiwallet methods instead
        Err(anyhow!("close_position_with_fallback is deprecated"))
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

    fn calculate_dynamic_thresholds(&self, _prices: &[PriceFeed]) -> crate::strategy::DynamicThresholds {
        // This function is not used but kept for compatibility
        crate::strategy::DynamicThresholds {
            rsi_oversold: 30.0,
            rsi_overbought: 70.0,
            momentum_threshold: 0.003,
            volatility_multiplier: 1.0,
            market_regime: crate::strategy::MarketRegime::Consolidating,
            trend_strength: 0.0,
            support_level: None,
            resistance_level: None,
        }
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

    async fn trading_cycle(&mut self) -> Result<()> {
        // Step 1: Fetch price data
        let prices = self.fetch_price_history().await?;
        
        if prices.is_empty() {
            warn!("⚠️ No price data available");
            return Ok(());
        }

        // Step 2: Fetch technical indicators
        let consolidated_indicators = match self.fetch_technical_indicators().await {
            Ok(indicators) => indicators,
            Err(_) => {
                // Fallback: calculate indicators from price data
                let strategy_indicators = self.strategy.calculate_custom_indicators(&prices);
                self.calculate_consolidated_indicators(&prices, &strategy_indicators)
            }
        };

        // Step 3: Calculate strategy indicators
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
        
        // Step 4.1: Enhance signal with ML predictions
        let enhanced_signal = match self.ml_strategy.enhance_signal(&signal, &prices, &strategy_indicators) {
            Ok(enhanced) => {
                if enhanced.signal_type != signal.signal_type || (enhanced.confidence - signal.confidence).abs() > 0.1 {
                    info!("🤖 ML enhanced: {:?} ({}%) → {:?} ({}%)", 
                          signal.signal_type, (signal.confidence * 100.0) as i32,
                          enhanced.signal_type, (enhanced.confidence * 100.0) as i32);
                }
                enhanced
            }
            Err(e) => {
                warn!("⚠️ ML enhancement failed: {} - using original signal", e);
                signal
            }
        };

        // Step 4.6: Post trading signal to database
        if let Err(e) = self.post_trading_signal(&enhanced_signal).await {
            warn!("Failed to post signal: {}", e);
        }

        // Step 5: MULTIWALLET EXECUTION - Handle signal across all wallets
        match enhanced_signal.signal_type {
            SignalType::Buy => {
                self.handle_multiwallet_buy_signal(&enhanced_signal).await?;
            }
            SignalType::Sell => {
                self.handle_multiwallet_sell_signal(&enhanced_signal).await?;
            }
            SignalType::Hold => {
                // Check for exit conditions on existing positions
                self.check_multiwallet_exit_conditions(&prices, &strategy_indicators).await?;
            }
        }
        
        // Step 6: Log the analysis
        self.log_analysis(&enhanced_signal, &prices, &consolidated_indicators);

        Ok(())
    }

    async fn handle_multiwallet_buy_signal(&mut self, signal: &TradingSignal) -> Result<()> {
        info!("🟢 Processing BUY signal across {} wallets", self.trading_executors.len());
        
        // Find first available wallet (has USDC balance and no open position)
        for (wallet_index, executor) in self.trading_executors.iter().enumerate() {
            if self.can_wallet_buy(wallet_index).await? {
                info!("💰 {} executing BUY signal", executor.get_wallet_name());
                
                match executor.execute_signal(signal, None).await {
                    Ok((success, quantity)) => {
                        if success {
                            // Create position record
                            let position = Position {
                                position_id: None, // Will be set after database post
                                entry_price: signal.price,
                                entry_time: signal.timestamp,
                                quantity: quantity.unwrap_or(1.0),
                                position_type: PositionType::Long,
                            };

                            // Post to database and get position ID
                            match self.post_position_for_wallet(&position, wallet_index).await {
                                Ok(position_id) => {
                                    let mut updated_position = position;
                                    updated_position.position_id = Some(position_id);
                                    self.wallet_positions[wallet_index] = Some(updated_position);
                                    
                                    info!("✅ {} opened position at ${:.4}", 
                                          executor.get_wallet_name(), signal.price);
                                    return Ok(()); // Exit after first successful execution
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
            }
        }
        
        info!("💤 No available wallets for BUY signal");
        Ok(())
    }

    async fn handle_multiwallet_sell_signal(&mut self, signal: &TradingSignal) -> Result<()> {
        info!("🔴 Processing SELL signal across {} wallets", self.trading_executors.len());
        
        let mut positions_closed = 0;
        
        // Check all wallets for open positions to close
        for (wallet_index, executor) in self.trading_executors.iter().enumerate() {
            if let Some(position) = &self.wallet_positions[wallet_index] {
                info!("💱 {} closing position opened at ${:.4}", 
                      executor.get_wallet_name(), position.entry_price);
                
                match executor.execute_signal(signal, Some(position.quantity)).await {
                    Ok((success, _)) => {
                        if success {
                            // Calculate PnL
                            let pnl = (signal.price - position.entry_price) / position.entry_price;
                            
                            // Close position in database
                            if let Err(e) = self.close_position_in_database_for_wallet(
                                wallet_index, signal.price
                            ).await {
                                error!("❌ Failed to close position in database for {}: {}", 
                                       executor.get_wallet_name(), e);
                            }

                            // Record trade result for ML
                            let trade_result = TradeResult {
                                entry_price: position.entry_price,
                                exit_price: signal.price,
                                pnl,
                                duration_seconds: (signal.timestamp - position.entry_time).num_seconds(),
                                entry_time: position.entry_time,
                                exit_time: signal.timestamp,
                                success: pnl > 0.0,
                                usdc_spent: None, // Legacy code - USDC tracking not available
                                usdc_received: None,
                                usdc_pnl: None,
                            };
                            self.ml_strategy.record_trade(trade_result);

                            // Clear position
                            self.wallet_positions[wallet_index] = None;
                            positions_closed += 1;
                            
                            let pnl_emoji = if pnl > 0.0 { "💰" } else { "💸" };
                            info!("✅ {} closed position: {} PnL: {:.2}%", 
                                  executor.get_wallet_name(), pnl_emoji, pnl * 100.0);
                        }
                    }
                    Err(e) => {
                        warn!("⚠️ {} failed to execute SELL: {}", executor.get_wallet_name(), e);
                    }
                }
            }
        }
        
        if positions_closed == 0 {
            info!("💤 No open positions to close");
        } else {
            info!("✅ Closed {} positions", positions_closed);
        }
        
        Ok(())
    }

    async fn can_wallet_buy(&self, wallet_index: usize) -> Result<bool> {
        // Check if wallet has no open position
        if self.wallet_positions[wallet_index].is_some() {
            return Ok(false);
        }

        // For now, assume wallet can buy if no position
        // In a full implementation, you'd check USDC balance here
        Ok(true)
    }

    async fn check_multiwallet_exit_conditions(&mut self, prices: &[PriceFeed], indicators: &crate::models::TradingIndicators) -> Result<()> {
        // Collect positions to check (to avoid borrowing issues)
        let mut positions_to_check = Vec::new();
        for (wallet_index, position) in self.wallet_positions.iter().enumerate() {
            if let Some(pos) = position {
                positions_to_check.push((wallet_index, pos.clone()));
            }
        }
        
        // Check each position for exit conditions
        for (wallet_index, pos) in positions_to_check {
            let executor = &self.trading_executors[wallet_index];
            let current_price = prices.last().map(|p| p.price).unwrap_or(0.0);
            let pnl = (current_price - pos.entry_price) / pos.entry_price;
            
            // Simple exit conditions (can be enhanced)
            let should_exit = 
                pnl > 0.05 ||  // 5% profit
                pnl < -0.03 || // 3% loss
                self.strategy.detect_momentum_decay(prices) ||
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
                if let Ok((success, _)) = executor.execute_signal(&exit_signal, Some(pos.quantity)).await {
                    if success {
                        // Handle position closure
                        if let Err(e) = self.close_position_in_database_for_wallet(wallet_index, current_price).await {
                            error!("❌ Failed to close position in database: {}", e);
                        }
                        
                        let trade_result = TradeResult {
                            entry_price: pos.entry_price,
                            exit_price: current_price,
                            pnl,
                            duration_seconds: (Utc::now() - pos.entry_time).num_seconds(),
                            entry_time: pos.entry_time,
                            exit_time: Utc::now(),
                            success: pnl > 0.0,
                            usdc_spent: None, // Legacy code - USDC tracking not available
                            usdc_received: None,
                            usdc_pnl: None,
                        };
                        self.ml_strategy.record_trade(trade_result);
                        
                        // Clear position
                        self.wallet_positions[wallet_index] = None;
                        
                        let pnl_emoji = if pnl > 0.0 { "💰" } else { "💸" };
                        info!("✅ {} exit completed: {} PnL: {:.2}%", 
                              executor.get_wallet_name(), pnl_emoji, pnl * 100.0);
                    }
                }
            }
        }
        
        Ok(())
    }

    // DUPLICATE METHOD REMOVED - using the first definition

    // Helper method to get wallet statistics
    pub fn get_wallet_stats(&self) -> Vec<WalletStats> {
        self.trading_executors.iter().enumerate().map(|(i, executor)| {
            WalletStats {
                wallet_index: i,
                wallet_name: executor.get_wallet_name().to_string(),
                has_position: self.wallet_positions[i].is_some(),
                position_entry_price: self.wallet_positions[i].as_ref().map(|p| p.entry_price),
                position_age_hours: self.wallet_positions[i].as_ref().map(|p| {
                    (Utc::now() - p.entry_time).num_hours()
                }),
            }
        }).collect()
    }

    async fn fetch_price_history(&self) -> Result<Vec<PriceFeed>> {
        use urlencoding::encode;
        // Try to fetch 1-minute candles first for better analysis
        let candle_url = format!("{}/candles/{}/1m?limit=200", 
                                self.config.database_url, 
                                encode(&self.config.trading_pair));
        
        let response = self.client.get(&candle_url).send().await?;
        let text = response.text().await?;
        if text.trim().is_empty() {
            warn!("Candle endpoint returned empty response");
        }
        let api_response: Result<crate::models::ApiResponse<Vec<crate::models::Candle>>, _> = serde_json::from_str(&text);
        if let Ok(api_response) = api_response {
            match api_response {
                crate::models::ApiResponse { success: true, data: Some(candles), .. } => {
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

    // OLD METHOD - REMOVED FOR MULTIWALLET
    // This method has been replaced by multiwallet signal handling methods
    async fn execute_signal_deprecated(&mut self, _signal: &TradingSignal) -> Result<()> {
        return Err(anyhow!("execute_signal is deprecated - use multiwallet signal handling methods"));
    }

    // This method is deprecated - all signal execution is now handled by multiwallet methods in trading_cycle
    async fn execute_signal(&mut self, _signal: &TradingSignal) -> Result<()> {
        return Err(anyhow!("execute_signal is deprecated - signal execution is handled in trading_cycle"));
    }

    fn calculate_pnl(&self, current_price: f64, position: &Position) -> f64 {
        match position.position_type {
            PositionType::Long => (current_price - position.entry_price) / position.entry_price,
            PositionType::Short => (position.entry_price - current_price) / position.entry_price,
        }
    }

    async fn open_position(&mut self, price: f64, position_type: PositionType, quantity: f64) -> Result<()> {
        // DEPRECATED: This method is no longer used in multiwallet implementation
        return Err(anyhow!("open_position is deprecated - use multiwallet position handling"));
    }

    // DEPRECATED: This method is no longer used in multiwallet implementation
    async fn close_position(&mut self, _price: f64) -> Result<()> {
        return Err(anyhow!("close_position is deprecated - use multiwallet position handling"));
    }

}
