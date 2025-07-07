use crate::models::{TradingSignal, SignalType, TradingIndicators, PriceFeed};
use crate::config::Config;
use chrono::{DateTime, Utc};
use tracing::{info, warn, debug};

pub struct TradingStrategy {
    config: Config,
    last_signal: Option<TradingSignal>,
}

impl TradingStrategy {
    pub fn new(config: Config) -> Self {
        Self {
            config,
            last_signal: None,
        }
    }

    pub fn analyze(&mut self, prices: &[PriceFeed], indicators: &crate::models::TechnicalIndicators) -> TradingSignal {
        let current_price = indicators.current_price;
        let timestamp = indicators.timestamp;

        // Calculate our custom indicators
        let custom_indicators = self.calculate_custom_indicators(prices);
        
        // Generate trading signal
        let signal = self.generate_signal(
            current_price,
            timestamp,
            &custom_indicators,
            indicators,
        );

        self.last_signal = Some(signal.clone());
        signal
    }

    fn calculate_custom_indicators(&self, prices: &[PriceFeed]) -> TradingIndicators {
        if prices.len() < self.config.rsi_slow_period {
            return TradingIndicators {
                rsi_fast: None,
                rsi_slow: None,
                sma_short: None,
                sma_long: None,
                volatility: None,
                price_momentum: None,
                price_change_percent: 0.0,
            };
        }

        let price_values: Vec<f64> = prices.iter().map(|p| p.price).collect();
        
        // Calculate RSI for different periods
        let rsi_fast = self.calculate_rsi(&price_values, self.config.rsi_fast_period);
        let rsi_slow = self.calculate_rsi(&price_values, self.config.rsi_slow_period);
        
        // Calculate SMAs
        let sma_short = self.calculate_sma(&price_values, self.config.sma_short_period);
        let sma_long = self.calculate_sma(&price_values, self.config.sma_long_period);
        
        // Calculate volatility
        let volatility = self.calculate_volatility(&price_values, self.config.volatility_window);
        
        // Calculate price momentum (rate of change)
        let price_momentum = self.calculate_price_momentum(&price_values);
        
        // Calculate price change percentage
        let price_change_percent = if prices.len() >= 2 {
            let current = prices[prices.len() - 1].price;
            let previous = prices[prices.len() - 2].price;
            (current - previous) / previous
        } else {
            0.0
        };

        TradingIndicators {
            rsi_fast,
            rsi_slow,
            sma_short,
            sma_long,
            volatility,
            price_momentum,
            price_change_percent,
        }
    }

    fn generate_signal(
        &self,
        current_price: f64,
        timestamp: DateTime<Utc>,
        indicators: &TradingIndicators,
        db_indicators: &crate::models::TechnicalIndicators,
    ) -> TradingSignal {
        let mut confidence: f64 = 0.0;
        let mut reasoning = Vec::new();
        let mut signal_type = SignalType::Hold;

        // Strategy 1: RSI Divergence Signal
        if let (Some(rsi_fast), Some(rsi_slow)) = (indicators.rsi_fast, indicators.rsi_slow) {
            // Buy signal: Fast RSI crosses above slow RSI from oversold
            if rsi_fast > rsi_slow && rsi_fast < 40.0 && rsi_slow < 35.0 {
                signal_type = SignalType::Buy;
                confidence += 0.3;
                reasoning.push(format!("RSI divergence: Fast RSI ({:.2}) > Slow RSI ({:.2}) from oversold", rsi_fast, rsi_slow));
            }
            
            // Sell signal: Fast RSI crosses below slow RSI from overbought
            if rsi_fast < rsi_slow && rsi_fast > 60.0 && rsi_slow > 65.0 {
                signal_type = SignalType::Sell;
                confidence += 0.3;
                reasoning.push(format!("RSI divergence: Fast RSI ({:.2}) < Slow RSI ({:.2}) from overbought", rsi_fast, rsi_slow));
            }
        }

        // Strategy 2: Moving Average Crossover
        if let (Some(sma_short), Some(sma_long)) = (indicators.sma_short, indicators.sma_long) {
            let ma_ratio = sma_short / sma_long;
            
            // Strong uptrend
            if ma_ratio > 1.02 && current_price > sma_short {
                if signal_type == SignalType::Buy {
                    confidence += 0.2;
                } else {
                    signal_type = SignalType::Buy;
                    confidence += 0.2;
                }
                reasoning.push(format!("Strong uptrend: SMA ratio {:.3}, price above short SMA", ma_ratio));
            }
            
            // Strong downtrend
            if ma_ratio < 0.98 && current_price < sma_short {
                if signal_type == SignalType::Sell {
                    confidence += 0.2;
                } else {
                    signal_type = SignalType::Sell;
                    confidence += 0.2;
                }
                reasoning.push(format!("Strong downtrend: SMA ratio {:.3}, price below short SMA", ma_ratio));
            }
        }

        // Strategy 3: Volatility Breakout
        if let Some(volatility) = indicators.volatility {
            let avg_volatility = 0.02; // 2% average volatility
            
            if volatility > avg_volatility * 1.5 {
                // High volatility - look for momentum continuation
                if indicators.price_momentum.unwrap_or(0.0) > self.config.price_change_threshold {
                    if signal_type == SignalType::Buy {
                        confidence += 0.15;
                    } else {
                        signal_type = SignalType::Buy;
                        confidence += 0.15;
                    }
                    reasoning.push(format!("Volatility breakout: {:.3}% volatility with positive momentum", volatility * 100.0));
                } else if indicators.price_momentum.unwrap_or(0.0) < -self.config.price_change_threshold {
                    if signal_type == SignalType::Sell {
                        confidence += 0.15;
                    } else {
                        signal_type = SignalType::Sell;
                        confidence += 0.15;
                    }
                    reasoning.push(format!("Volatility breakout: {:.3}% volatility with negative momentum", volatility * 100.0));
                }
            }
        }

        // Strategy 4: Mean Reversion (when RSI is extreme)
        if let Some(rsi_fast) = indicators.rsi_fast {
            if rsi_fast < 20.0 && signal_type == SignalType::Hold {
                signal_type = SignalType::Buy;
                confidence += 0.25;
                reasoning.push(format!("Mean reversion: Extreme oversold RSI ({:.2})", rsi_fast));
            } else if rsi_fast > 80.0 && signal_type == SignalType::Hold {
                signal_type = SignalType::Sell;
                confidence += 0.25;
                reasoning.push(format!("Mean reversion: Extreme overbought RSI ({:.2})", rsi_fast));
            }
        }

        // Strategy 5: Price Momentum Confirmation
        let momentum_threshold = self.config.price_change_threshold;
        if indicators.price_change_percent.abs() > momentum_threshold {
            if indicators.price_change_percent > momentum_threshold && signal_type == SignalType::Buy {
                confidence += 0.1;
                reasoning.push(format!("Momentum confirmation: {:.2}% price increase", indicators.price_change_percent * 100.0));
            } else if indicators.price_change_percent < -momentum_threshold && signal_type == SignalType::Sell {
                confidence += 0.1;
                reasoning.push(format!("Momentum confirmation: {:.2}% price decrease", indicators.price_change_percent * 100.0));
            }
        }

        // Cap confidence at 1.0
        confidence = confidence.min(1.0_f64);

        // Only generate signals if confidence is high enough
        if confidence < 0.4 {
            signal_type = SignalType::Hold;
            reasoning.push("Insufficient confidence for trade signal".to_string());
        }

        TradingSignal {
            signal_type,
            confidence,
            price: current_price,
            timestamp,
            reasoning: reasoning.join("; "),
            indicators: indicators.clone(),
        }
    }

    fn calculate_rsi(&self, prices: &[f64], period: usize) -> Option<f64> {
        if prices.len() < period + 1 {
            return None;
        }

        let mut gains = 0.0;
        let mut losses = 0.0;

        for i in (prices.len() - period)..prices.len() {
            let change = prices[i] - prices[i - 1];
            if change > 0.0 {
                gains += change;
            } else {
                losses += change.abs();
            }
        }

        let avg_gain = gains / period as f64;
        let avg_loss = losses / period as f64;

        if avg_loss == 0.0 {
            return Some(100.0);
        }

        let rs = avg_gain / avg_loss;
        let rsi = 100.0 - (100.0 / (1.0 + rs));
        Some(rsi)
    }

    fn calculate_sma(&self, prices: &[f64], period: usize) -> Option<f64> {
        if prices.len() < period {
            return None;
        }
        
        let sum: f64 = prices.iter().rev().take(period).sum();
        Some(sum / period as f64)
    }

    fn calculate_volatility(&self, prices: &[f64], window: usize) -> Option<f64> {
        if prices.len() < window + 1 {
            return None;
        }

        let returns: Vec<f64> = prices
            .windows(2)
            .map(|w| (w[1] - w[0]) / w[0])
            .collect();

        let mean = returns.iter().sum::<f64>() / returns.len() as f64;
        let variance = returns.iter()
            .map(|r| (r - mean).powi(2))
            .sum::<f64>() / returns.len() as f64;
        
        Some(variance.sqrt())
    }

    fn calculate_price_momentum(&self, prices: &[f64]) -> Option<f64> {
        if prices.len() < 5 {
            return None;
        }

        let recent = prices[prices.len() - 1];
        let older = prices[prices.len() - 5];
        Some((recent - older) / older)
    }
} 