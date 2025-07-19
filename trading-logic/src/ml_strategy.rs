use crate::models::{PriceFeed, TradingSignal, SignalType};
use crate::config::Config;
use crate::neural_enhancement::{NeuralEnhancement, TradeOutcome};
use anyhow::{Result, anyhow};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use tracing::{info, warn, debug};
use reqwest::Client;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MLFeatures {
    pub rsi_fast: f64,           // Current RSI (normalized 0-1)
    pub win_rate: f64,           // Recent win rate (0-1)
    pub consecutive_losses: f64,  // Number of consecutive losses
    pub volatility: f64,         // Current volatility
}

#[derive(Debug, Clone)]
pub struct MLPrediction {
    pub entry_probability: f64,
    pub exit_probability: f64,
    pub confidence_score: f64,
    pub market_regime: MarketRegime,
    pub optimal_position_size: f64, // 0.0 to 1.0 (percentage of 90%)
    pub risk_score: f64, // 0.0 to 1.0 (higher = riskier)
    pub win_rate: f64, // Recent win rate
    pub consecutive_losses: f64, // Number of consecutive losses
}

#[derive(Debug, Clone, PartialEq)]
pub enum MarketRegime {
    Consolidating,
    Trending,
    Volatile,
    Unknown,
}

#[derive(Debug)]
pub struct MLStrategy {
    config: Config,
    recent_trades: VecDeque<TradeResult>,
    ml_enabled: bool,
    min_confidence_threshold: f64,
    max_position_size: f64,
    db_client: Client,
    database_url: String,
    
    // Neural network integration
    neural_system: Option<NeuralEnhancement>,
    neural_enabled: bool,
}

#[derive(Debug, Clone)]
pub struct TradeResult {
    pub entry_price: f64,
    pub exit_price: f64,
    pub pnl: f64, // Percentage-based P&L for compatibility
    pub duration_seconds: i64,
    pub entry_time: DateTime<Utc>,
    pub exit_time: DateTime<Utc>,
    pub success: bool,
    // USDC-based tracking for accurate P&L
    pub usdc_spent: Option<f64>,
    pub usdc_received: Option<f64>,
    pub usdc_pnl: Option<f64>, // Actual USDC profit/loss
}

impl MLStrategy {
    pub fn new(config: Config) -> Self {
        let ml_enabled = std::env::var("ML_ENABLED")
            .unwrap_or_else(|_| "true".to_string())
            .parse::<bool>()
            .unwrap_or(true);

        let min_confidence_threshold = std::env::var("MIN_CONFIDENCE_THRESHOLD")
            .unwrap_or_else(|_| "0.55".to_string())
            .parse::<f64>()
            .unwrap_or(0.55);

        let max_position_size = std::env::var("ML_MAX_POSITION_SIZE")
            .unwrap_or_else(|_| "0.9".to_string())
            .parse::<f64>()
            .unwrap_or(0.9);

        let neural_enabled = std::env::var("NEURAL_ENABLED")
            .unwrap_or_else(|_| "true".to_string())
            .parse::<bool>()
            .unwrap_or(true);

        // Initialize neural system
        let neural_system = if neural_enabled {
            match NeuralEnhancement::new() {
                Ok(system) => {
                    info!("🧠 Neural Enhancement System initialized successfully");
                    Some(system)
                }
                Err(e) => {
                    warn!("⚠️ Failed to initialize neural system: {}", e);
                    None
                }
            }
        } else {
            info!("🧠 Neural Enhancement System disabled by configuration");
            None
        };

        Self {
            config,
            recent_trades: VecDeque::new(),
            ml_enabled,
            min_confidence_threshold,
            max_position_size,
            db_client: Client::new(),
            database_url: std::env::var("DATABASE_URL").unwrap_or_else(|_| "http://localhost:8080".to_string()),
            neural_system,
            neural_enabled,
        }
    }

    pub async fn enhance_signal(&mut self, signal: &TradingSignal, prices: &[PriceFeed], indicators: &crate::models::TradingIndicators) -> Result<TradingSignal> {
        if !self.ml_enabled {
            return Ok(signal.clone());
        }

        // Auto-save ML trade data every 5 trades to prevent data loss
        if self.recent_trades.len() % 5 == 0 && !self.recent_trades.is_empty() {
            if let Err(e) = self.save_recent_trades_to_database().await {
                warn!("⚠️ Failed to auto-save ML trade data: {}", e);
            } else {
                debug!("💾 ML trade data auto-saved ({} trades)", self.recent_trades.len());
            }
        }

        let features = self.extract_features(prices, indicators)?;
        
        // Handle no-trade-history scenario more gracefully
        if self.recent_trades.is_empty() {
            debug!("🤖 ML: No trade history available, using minimal adjustments");
            let mut enhanced_signal = signal.clone();
            
            // Only apply volatility adjustments when no trade history
            if features.volatility > 0.25 {
                enhanced_signal.confidence -= 0.05; // Minimal reduction for very high volatility
                enhanced_signal.reasoning.push("ML: High volatility adjustment (no trade history)".to_string());
            }
            
            // Add basic ML reasoning
            enhanced_signal.reasoning.push("ML: No trade history - minimal adjustments applied".to_string());
            
            // Apply neural enhancement if available
            if let Some(ref mut neural_system) = self.neural_system {
                match neural_system.enhance_signal(&enhanced_signal, prices, indicators).await {
                    Ok(neural_enhanced_signal) => {
                        enhanced_signal = neural_enhanced_signal;
                        info!("🧠 Neural enhancement applied successfully (no ML history)");
                    }
                    Err(e) => {
                        warn!("Neural signal enhancement failed: {}", e);
                    }
                }
            }
            
            return Ok(enhanced_signal);
        }

        let prediction = self.predict(&features)?;

        // Only log significant ML predictions
        if prediction.confidence_score > 0.7 || prediction.confidence_score < 0.3 {
            info!("🤖 ML: Win Rate {:.0}% | Losses {:.0} | Conf {:.0}% | Regime: {:?}", 
                  prediction.win_rate * 100.0, 
                  prediction.consecutive_losses, 
                  prediction.confidence_score * 100.0,
                  prediction.market_regime);
        }

        // Start with the base signal
        let mut enhanced_signal = signal.clone();
        
        // Simple confidence adjustments based on ML prediction (more conservative)
        if prediction.win_rate > 0.7 {
            enhanced_signal.confidence += 0.05; // Smaller boost if winning consistently
        } else if prediction.win_rate > 0.6 {
            enhanced_signal.confidence += 0.03; // Even smaller boost if winning
        } else if prediction.win_rate < 0.3 {
            enhanced_signal.confidence -= 0.05; // Smaller reduction if losing consistently
        } else if prediction.win_rate < 0.4 {
            enhanced_signal.confidence -= 0.03; // Even smaller reduction if losing
        }
        
        // Additional adjustments for consecutive losses (less aggressive)
        if prediction.consecutive_losses > 5.0 {
            enhanced_signal.confidence -= 0.05; // Small reduction only after many losses
        } else if prediction.consecutive_losses > 3.0 {
            enhanced_signal.confidence -= 0.03; // Very small reduction after losses
        }
        // Note: Removed penalty for 2-3 losses to prevent getting stuck
        
        // Volatility adjustment (crypto-appropriate thresholds)
        if features.volatility > 0.25 {
            enhanced_signal.confidence -= 0.08; // Moderate reduction in very high volatility (25%+)
        } else if features.volatility > 0.15 {
            enhanced_signal.confidence -= 0.05; // Smaller reduction in high volatility (15%+)
        }
        // Note: 7% volatility (your current level) now gets no penalty
        
        // Cap confidence at reasonable bounds
        enhanced_signal.confidence = enhanced_signal.confidence.max(0.2).min(0.9);
        
        // Convert to HOLD if confidence too low
        if enhanced_signal.confidence < self.min_confidence_threshold {
            enhanced_signal.signal_type = SignalType::Hold;
            enhanced_signal.reasoning.push(format!("ML confidence too low ({:.0}% < {:.0}%) - converted to HOLD", 
                  enhanced_signal.confidence * 100.0, self.min_confidence_threshold * 100.0));
        }
        
        // Add ML reasoning
        enhanced_signal.reasoning.push(format!("ML Win Rate: {:.0}%", prediction.win_rate * 100.0));
        enhanced_signal.reasoning.push(format!("ML Consecutive Losses: {:.0}", prediction.consecutive_losses));
        enhanced_signal.reasoning.push(format!("ML Market Regime: {:?}", prediction.market_regime));
        enhanced_signal.reasoning.push(format!("ML Risk Score: {:.0}%", prediction.risk_score * 100.0));
        
        // 🧠 Neural Network Enhancement
        if let Some(ref mut neural_system) = self.neural_system {
            match neural_system.enhance_signal(&enhanced_signal, prices, indicators).await {
                Ok(neural_enhanced_signal) => {
                    enhanced_signal = neural_enhanced_signal;
                    info!("🧠 Neural enhancement applied successfully");
                }
                Err(e) => {
                    warn!("Neural signal enhancement failed: {}", e);
                }
            }
        }
        
        Ok(enhanced_signal)
    }

    fn extract_features(&self, prices: &[PriceFeed], indicators: &crate::models::TradingIndicators) -> Result<MLFeatures> {
        // Calculate performance metrics from recent trades
        let (win_rate, consecutive_losses) = self.calculate_performance_metrics();
        
        let features = MLFeatures {
            rsi_fast: indicators.rsi_fast.unwrap_or(50.0) / 100.0, // Normalize to 0-1
            win_rate,
            consecutive_losses,
            volatility: indicators.volatility.unwrap_or(0.02),
        };
        Ok(features)
    }

    fn predict(&self, features: &MLFeatures) -> Result<MLPrediction> {
        // Simple ML model using only essential features
        let win_rate = features.win_rate;
        let consecutive_losses = features.consecutive_losses;
        let volatility = features.volatility;
        
        // More stable confidence calculation
        let mut confidence_score: f64 = 0.5; // Base confidence
        
        // Adjust confidence based on performance (more conservative)
        if win_rate > 0.7 {
            confidence_score += 0.15; // High confidence if winning consistently
        } else if win_rate > 0.6 {
            confidence_score += 0.1; // Moderate confidence if winning
        } else if win_rate < 0.3 {
            confidence_score -= 0.15; // Low confidence if losing consistently
        } else if win_rate < 0.4 {
            confidence_score -= 0.1; // Moderate reduction if losing
        }
        
        // Adjust for consecutive losses (less aggressive)
        if consecutive_losses > 5.0 {
            confidence_score -= 0.1; // Small reduction only after many losses
        } else if consecutive_losses > 3.0 {
            confidence_score -= 0.05; // Very small reduction after losses
        }
        // Note: Removed penalty for 2-3 losses to prevent getting stuck
        
        // Adjust for volatility (crypto-appropriate thresholds)
        if volatility > 0.25 {
            confidence_score -= 0.15; // Bigger reduction in very high volatility (25%+)
        } else if volatility > 0.15 {
            confidence_score -= 0.1; // Moderate reduction in high volatility (15%+)
        }
        // Note: 7% volatility (your current level) now gets no penalty
        
        // Cap confidence at reasonable bounds
        confidence_score = confidence_score.max(0.2_f64).min(0.8_f64);
        
        // More conservative risk calculation
        let risk_score = if consecutive_losses > 3.0 { 0.7 } 
                        else if consecutive_losses > 2.0 { 0.5 }
                        else { 0.3 };
        
        // Crypto-appropriate market regime classification
        let market_regime = if volatility > 0.20 { 
            MarketRegime::Volatile  // 20%+ volatility is truly volatile for crypto
        } else if win_rate > 0.6 { 
            MarketRegime::Trending 
        } else { 
            MarketRegime::Consolidating 
        };

        Ok(MLPrediction {
            entry_probability: 0.5, // Neutral - let base strategy decide
            exit_probability: 0.5,  // Neutral - let base strategy decide
            confidence_score,
            market_regime,
            optimal_position_size: self.calculate_optimal_position_size(features),
            risk_score,
            win_rate,
            consecutive_losses,
        })
    }

    // Remove complex ML functions - no longer needed with simplified approach
    // fn calculate_entry_probability(&self, features: &MLFeatures) -> f64 { ... }
    // fn calculate_exit_probability(&self, features: &MLFeatures) -> f64 { ... }
    // fn calculate_confidence_score(&self, features: &MLFeatures) -> f64 { ... }
    // fn apply_ml_enhancements(&self, signal: &TradingSignal, prediction: &MLPrediction) -> Result<TradingSignal> { ... }

    fn calculate_optimal_position_size(&self, features: &MLFeatures) -> f64 {
        let mut size = self.max_position_size; // Start with max size
        
        // Reduce size based on risk factors
        if features.consecutive_losses > 2.0 { size *= 0.5; } // 50% reduction after losses
        if features.consecutive_losses > 4.0 { size *= 0.3; } // 70% reduction after many losses
        if features.volatility > 0.20 { size *= 0.6; } // 40% reduction in very high volatility (20%+)
        if features.win_rate < 0.4 { size *= 0.7; } // 30% reduction with poor performance
        
        // Increase size for high-confidence setups
        if features.win_rate > 0.7 { 
            size = self.max_position_size; // Full size for strong setups
        }
        
        size.max(0.05).min(self.max_position_size) // Min 5%, Max 90%
    }

    pub fn calculate_market_regime(&self, prices: &[PriceFeed]) -> (i32, f64) {
        if prices.len() < 20 {
            return (0, 0.0); // Consolidating
        }
        
        let price_values: Vec<f64> = prices.iter().map(|p| p.price).collect();
        let trend_strength = self.calculate_trend_strength(&price_values);
        let volatility = self.calculate_volatility(&price_values, 20).unwrap_or(0.02);
        
        let regime = if trend_strength > 0.7 { 1 } // Trending
                    else if volatility > 0.05 { 2 } // Volatile
                    else { 0 }; // Consolidating
        
        (regime, trend_strength)
    }

    pub fn calculate_trend_strength(&self, prices: &[f64]) -> f64 {
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
        
        let trend_strength = (slope / avg_price).abs();
        trend_strength.min(1.0)
    }

    pub fn calculate_volatility(&self, prices: &[f64], window: usize) -> Option<f64> {
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

    fn calculate_support_resistance_distances(&self, prices: &[PriceFeed], current_price: f64) -> (f64, f64) {
        if prices.len() < 20 {
            return (0.0, 0.0);
        }
        
        let price_values: Vec<f64> = prices.iter().map(|p| p.price).collect();
        let recent_prices = &price_values[price_values.len().saturating_sub(20)..];
        
        let support = recent_prices.iter().fold(f64::INFINITY, |a, &b| a.min(b));
        let resistance = recent_prices.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b));
        
        let support_distance = (current_price - support) / current_price;
        let resistance_distance = (resistance - current_price) / current_price;
        
        (support_distance, resistance_distance)
    }

    fn calculate_performance_metrics(&self) -> (f64, f64) {
        if self.recent_trades.is_empty() {
            return (0.5, 0.0); // Default to 50% win rate and 0 consecutive losses
        }
        
        let recent_trades: Vec<&TradeResult> = self.recent_trades.iter().take(10).collect();
        let total_trades = recent_trades.len() as f64;
        
        let winning_trades = recent_trades.iter().filter(|t| t.success).count() as f64;
        let win_rate = winning_trades / total_trades;
        
        // Add smoothing to prevent sudden jumps
        let smoothed_win_rate = if total_trades < 3.0 {
            // Use a more conservative estimate for small sample sizes
            0.4 + (win_rate * 0.2) // Range: 40-60% for small samples
        } else {
            win_rate
        };
        
        let consecutive_losses = self.calculate_consecutive_losses();
        
        (smoothed_win_rate, consecutive_losses)
    }

    fn calculate_consecutive_losses(&self) -> f64 {
        let mut consecutive = 0.0;
        for trade in self.recent_trades.iter().rev() {
            if !trade.success {
                consecutive += 1.0;
            } else {
                break;
            }
        }
        consecutive
    }

    pub async fn record_trade(&mut self, trade_result: TradeResult) {
        self.recent_trades.push_back(trade_result.clone());
        
        // Keep only last 200 trades (increased for multiwallet learning)
        if self.recent_trades.len() > 200 {
            self.recent_trades.pop_front();
        }
        
        // 🧠 Neural Network Learning from trade result - prioritize USDC-based P&L
        if let Some(ref mut neural_system) = self.neural_system {
            // Use USDC-based P&L if available, otherwise fall back to percentage-based
            let actual_pnl = if let Some(usdc_pnl) = trade_result.usdc_pnl {
                // Convert USDC P&L to percentage for neural network compatibility
                if let Some(usdc_spent) = trade_result.usdc_spent {
                    usdc_pnl / usdc_spent.abs()
                } else {
                    trade_result.pnl // Fallback to percentage-based
                }
            } else {
                trade_result.pnl
            };
            
            let actual_success = if let Some(usdc_pnl) = trade_result.usdc_pnl {
                usdc_pnl > 0.0 // USDC-based success determination
            } else {
                trade_result.success
            };
            
            if trade_result.usdc_pnl.is_some() {
                info!("🧠 Neural Learning: Using USDC-based P&L: ${:.2} ({:.2}%)", 
                      trade_result.usdc_pnl.unwrap(), actual_pnl * 100.0);
            } else {
                info!("🧠 Neural Learning: Using percentage-based P&L: {:.2}%", actual_pnl * 100.0);
            }
            
            let trade_outcome = TradeOutcome {
                entry_price: trade_result.entry_price,
                exit_price: trade_result.exit_price,
                pnl: actual_pnl,
                success: actual_success,
                timestamp: trade_result.exit_time,
            };
            
            if let Err(e) = neural_system.learn_from_trade(&trade_outcome).await {
                warn!("Neural learning from trade failed: {}", e);
            }
        }
        
        // 💾 Always save trade to database for ML persistence
        let market_regime = "Recorded";
        let trend_strength = 0.5; // Default values
        let volatility = 0.05;
        
        if let Err(e) = self.save_trade_to_database(&trade_result, &self.config.trading_pair, market_regime, trend_strength, volatility).await {
            warn!("⚠️ Failed to save ML trade to database: {}", e);
        } else {
            info!("💾 ML trade saved to database for persistence");
        }
        
        info!("🤖 ML+Neural Trade Recorded - PnL: {:.2}%, Success: {}, Neural: {}", 
              trade_result.pnl * 100.0, 
              trade_result.success,
              self.neural_system.is_some());
    }

    pub async fn record_trade_with_context(&mut self, trade_result: TradeResult, pair: &str, market_regime: &str, trend_strength: f64, volatility: f64) {
        // Add to memory
        self.recent_trades.push_back(trade_result.clone());
        
        // Keep only last 200 trades (increased for multiwallet learning)
        if self.recent_trades.len() > 200 {
            self.recent_trades.pop_front();
        }
        
        // Save to database
        if let Err(e) = self.save_trade_to_database(&trade_result, pair, market_regime, trend_strength, volatility).await {
            warn!("⚠️ Failed to save trade to database: {}", e);
        }
        
        info!("🤖 ML Trade Recorded & Saved - PnL: {:.2}%, Success: {}", trade_result.pnl * 100.0, trade_result.success);
    }

    pub fn get_ml_stats(&self) -> MLStats {
        let total_trades = self.recent_trades.len();
        let winning_trades = self.recent_trades.iter().filter(|t| t.success).count();
        let win_rate = if total_trades > 0 { winning_trades as f64 / total_trades as f64 } else { 0.0 };
        
        let avg_pnl = if total_trades > 0 {
            self.recent_trades.iter().map(|t| t.pnl).sum::<f64>() / total_trades as f64
        } else {
            0.0
        };
        
        MLStats {
            total_trades,
            win_rate,
            avg_pnl,
            ml_enabled: self.ml_enabled,
            min_confidence_threshold: self.min_confidence_threshold,
            consecutive_losses: self.calculate_consecutive_losses(),
            current_volatility: avg_pnl.abs(), // Use average PnL as volatility proxy
        }
    }

    pub async fn load_trade_history(&mut self, pair: &str) -> Result<()> {
        // URL encode the pair to handle special characters like "/"
        let encoded_pair = urlencoding::encode(pair);
        let url = format!("{}/ml/trades/{}?limit=200", self.database_url, encoded_pair);
        
        info!("🔍 Loading ML trade history from: {}", url);
        
        match self.db_client.get(&url).send().await {
            Ok(response) => {
                if response.status().is_success() {
                    if let Ok(data) = response.json::<serde_json::Value>().await {
                        if let Some(trades_data) = data["data"].as_array() {
                            for trade_data in trades_data {
                                if let Ok(trade) = self.parse_trade_from_json(trade_data) {
                                    self.recent_trades.push_back(trade);
                                }
                            }
                            info!("🤖 Loaded {} ML trades from database for learning", self.recent_trades.len());
                            if self.recent_trades.is_empty() {
                                info!("📊 No existing ML trade history found - starting fresh");
                            }
                        } else {
                            info!("📊 No ML trade data array found in response");
                        }
                    } else {
                        warn!("⚠️ Failed to parse ML trade history response as JSON");
                    }
                } else {
                    warn!("⚠️ Failed to load ML trade history: HTTP {} from {}", response.status(), url);
                }
            }
            Err(e) => {
                warn!("⚠️ Failed to connect to database for ML trade history: {}", e);
                warn!("🔗 Database URL: {}", self.database_url);
            }
        }
        
        Ok(())
    }

    pub async fn save_trade_to_database(&self, trade: &TradeResult, pair: &str, market_regime: &str, trend_strength: f64, volatility: f64) -> Result<()> {
        let url = format!("{}/ml/trades", self.database_url);
        
        let payload = serde_json::json!({
            "id": Uuid::new_v4().to_string(),
            "pair": pair,
            "entry_price": trade.entry_price,
            "exit_price": trade.exit_price,
            "pnl": trade.pnl,
            "duration_seconds": trade.duration_seconds,
            "entry_time": trade.entry_time.to_rfc3339(),
            "exit_time": trade.exit_time.to_rfc3339(),
            "success": trade.success,
            "market_regime": market_regime,
            "trend_strength": trend_strength,
            "volatility": volatility,
            "created_at": Utc::now().to_rfc3339()
        });

        match self.db_client.post(&url).json(&payload).send().await {
            Ok(response) => {
                if response.status().is_success() {
                    info!("🤖 Saved trade to database: PnL {:.2}%, Success: {}", trade.pnl * 100.0, trade.success);
                } else {
                    warn!("⚠️ Failed to save trade to database: HTTP {}", response.status());
                }
            }
            Err(e) => {
                warn!("⚠️ Failed to save trade to database: {}", e);
            }
        }
        
        Ok(())
    }

    fn parse_trade_from_json(&self, trade_data: &serde_json::Value) -> Result<TradeResult> {
        let entry_price = trade_data["entry_price"].as_f64().ok_or_else(|| anyhow!("Missing entry_price"))?;
        let exit_price = trade_data["exit_price"].as_f64().ok_or_else(|| anyhow!("Missing exit_price"))?;
        let pnl = trade_data["pnl"].as_f64().ok_or_else(|| anyhow!("Missing pnl"))?;
        let duration_seconds = trade_data["duration_seconds"].as_i64().ok_or_else(|| anyhow!("Missing duration_seconds"))?;
        let entry_time = DateTime::parse_from_rfc3339(&trade_data["entry_time"].as_str().unwrap_or(""))?.with_timezone(&Utc);
        let exit_time = DateTime::parse_from_rfc3339(&trade_data["exit_time"].as_str().unwrap_or(""))?.with_timezone(&Utc);
        let success = trade_data["success"].as_bool().unwrap_or(false);

        Ok(TradeResult {
            entry_price,
            exit_price,
            pnl,
            duration_seconds,
            entry_time,
            exit_time,
            success,
            usdc_spent: trade_data.get("usdc_spent").and_then(|v| v.as_f64()),
            usdc_received: trade_data.get("usdc_received").and_then(|v| v.as_f64()),
            usdc_pnl: trade_data.get("usdc_pnl").and_then(|v| v.as_f64()),
        })
    }

    // Auto-save recent trades to database to prevent data loss
    async fn save_recent_trades_to_database(&self) -> Result<()> {
        if self.recent_trades.is_empty() {
            return Ok(());
        }

        // Save the last 5 trades that haven't been saved yet
        let trades_to_save: Vec<&TradeResult> = self.recent_trades.iter().rev().take(5).collect();
        
        for trade in trades_to_save {
            // Determine market regime and volatility for context
            let market_regime = "Auto-saved";
            let trend_strength = 0.5; // Default values for auto-save
            let volatility = 0.05;
            
            if let Err(e) = self.save_trade_to_database(trade, &self.config.trading_pair, market_regime, trend_strength, volatility).await {
                warn!("⚠️ Failed to auto-save trade: {}", e);
            }
        }
        
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct MLStats {
    pub total_trades: usize,
    pub win_rate: f64,
    pub avg_pnl: f64,
    pub ml_enabled: bool,
    pub min_confidence_threshold: f64,
    pub consecutive_losses: f64,
    pub current_volatility: f64,
} 