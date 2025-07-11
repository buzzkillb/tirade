use crate::models::{TradingSignal, SignalType, TradingIndicators, PriceFeed};
use crate::config::Config;
use chrono::{DateTime, Utc};
use tracing::info;

pub struct TradingStrategy {
    config: Config,
    last_signal: Option<TradingSignal>,
}

#[derive(Debug, Clone)]
pub struct DynamicThresholds {
    pub rsi_oversold: f64,
    pub rsi_overbought: f64,
    pub take_profit: f64,
    pub stop_loss: f64,
    pub momentum_threshold: f64,
    pub volatility_multiplier: f64,
    pub market_regime: MarketRegime,
    pub trend_strength: f64,
    pub support_level: Option<f64>,
    pub resistance_level: Option<f64>,
}

#[derive(Debug, Clone)]
pub enum MarketRegime {
    Trending,
    Ranging,
    Volatile,
    Consolidating,
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

        // Use realistic timeframes based on available data (30-second intervals)
        let short_term_prices = self.get_recent_prices(prices, 30 * 60); // 30 minutes
        let medium_term_prices = self.get_recent_prices(prices, 2 * 60 * 60); // 2 hours
        let long_term_prices = self.get_recent_prices(prices, 6 * 60 * 60); // 6 hours

        // Calculate indicators for different timeframes
        let short_term_indicators = self.calculate_custom_indicators(&short_term_prices);
        let medium_term_indicators = self.calculate_custom_indicators(&medium_term_prices);
        let long_term_indicators = self.calculate_custom_indicators(&long_term_prices);
        
        // Calculate dynamic thresholds using medium-term data for stability
        let dynamic_thresholds = self.calculate_dynamic_thresholds(&medium_term_prices, &medium_term_indicators);
        
        // Generate trading signal with multi-timeframe analysis
        let signal = self.generate_signal(
            current_price,
            timestamp,
            &short_term_indicators,
            &medium_term_indicators,
            &long_term_indicators,
            indicators,
            &dynamic_thresholds,
        );

        self.last_signal = Some(signal.clone());
        signal
    }

    fn calculate_dynamic_thresholds(&self, prices: &[PriceFeed], indicators: &TradingIndicators) -> DynamicThresholds {
        let volatility = indicators.volatility.unwrap_or(0.02);
        let price_momentum = indicators.price_momentum.unwrap_or(0.0);
        
        // Enhanced market analysis using price data only
        let (market_regime, trend_strength) = self.analyze_market_regime(prices, indicators);
        let (support_level, resistance_level) = self.calculate_support_resistance(prices);
        
        // Base thresholds (adaptive based on market regime) - INCREASED for better profitability
        let (base_rsi_oversold, base_rsi_overbought, base_take_profit, base_stop_loss, base_momentum_threshold) = 
            match market_regime {
                MarketRegime::Trending => (30.0, 70.0, 0.06, 0.035, 0.003), // Higher take profit, wider stop loss
                MarketRegime::Ranging => (35.0, 65.0, 0.04, 0.025, 0.004), // Increased take profit, wider stop loss
                MarketRegime::Volatile => (25.0, 75.0, 0.07, 0.04, 0.006),  // Higher take profit, wider stop loss
                MarketRegime::Consolidating => (40.0, 60.0, 0.035, 0.02, 0.002), // Increased take profit, wider stop loss
            };
        
        // Adjust based on volatility and trend strength
        let volatility_multiplier = if volatility > 0.05 {
            1.3 + (trend_strength * 0.2) // Higher multiplier in strong trends
        } else if volatility < 0.01 {
            0.8 - (trend_strength * 0.1) // Lower multiplier in weak trends
        } else {
            1.0 + (trend_strength * 0.1) // Normal with trend adjustment
        };
        
        // Adjust RSI thresholds based on market regime and volatility
        let rsi_oversold = base_rsi_oversold + (volatility * 100.0 * 0.5) - (trend_strength * 5.0);
        let rsi_overbought = base_rsi_overbought - (volatility * 100.0 * 0.5) + (trend_strength * 5.0);
        
        // Adjust take profit and stop loss based on volatility and support/resistance
        let take_profit = base_take_profit * volatility_multiplier;
        let stop_loss = base_stop_loss * volatility_multiplier;
        
        // Adjust momentum threshold based on market conditions
        let momentum_threshold = base_momentum_threshold * volatility_multiplier;
        
        info!("📊 Dynamic Thresholds - Regime: {:?}, Trend Strength: {:.2}, Volatility: {:.3}%, RSI Oversold: {:.1}, RSI Overbought: {:.1}, Take Profit: {:.2}%, Stop Loss: {:.2}%", 
              market_regime, trend_strength, volatility * 100.0, rsi_oversold, rsi_overbought, take_profit * 100.0, stop_loss * 100.0);
        
        DynamicThresholds {
            rsi_oversold,
            rsi_overbought,
            take_profit,
            stop_loss,
            momentum_threshold,
            volatility_multiplier,
            market_regime,
            trend_strength,
            support_level,
            resistance_level,
        }
    }

    pub fn calculate_custom_indicators(&self, prices: &[PriceFeed]) -> TradingIndicators {
        // Check if we have enough data for the longest indicator (SMA50 = 50 points)
        if prices.len() < self.config.sma_long_period {
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
        short_term_indicators: &TradingIndicators,
        medium_term_indicators: &TradingIndicators,
        long_term_indicators: &TradingIndicators,
        _db_indicators: &crate::models::TechnicalIndicators,
        dynamic_thresholds: &DynamicThresholds,
    ) -> TradingSignal {
        let mut confidence: f64 = 0.0;
        let mut reasoning = Vec::new();
        let mut signal_type = SignalType::Hold;

        // Debug: Log the indicator values for different timeframes
        info!("🔍 Strategy Debug - Short-term RSI: {:?}, Medium-term RSI: {:?}, Long-term RSI: {:?}", 
              short_term_indicators.rsi_fast, medium_term_indicators.rsi_fast, long_term_indicators.rsi_fast);
        info!("🔍 Strategy Debug - Short-term SMA: {:?}, Medium-term SMA: {:?}, Long-term SMA: {:?}", 
              short_term_indicators.sma_short, medium_term_indicators.sma_short, long_term_indicators.sma_short);

        // Strategy 1: RSI Divergence Signal (with dynamic thresholds)
        if let (Some(rsi_fast), Some(rsi_slow)) = (short_term_indicators.rsi_fast, short_term_indicators.rsi_slow) {
            // Buy signal: Fast RSI crosses above slow RSI from oversold
            if rsi_fast > rsi_slow && rsi_fast < dynamic_thresholds.rsi_oversold + 10.0 && rsi_slow < dynamic_thresholds.rsi_oversold + 5.0 {
                signal_type = SignalType::Buy;
                confidence += 0.3;
                reasoning.push(format!("RSI divergence: Fast RSI ({:.2}) > Slow RSI ({:.2}) from oversold (threshold: {:.1})", 
                                     rsi_fast, rsi_slow, dynamic_thresholds.rsi_oversold));
            }
            
            // Sell signal: Fast RSI crosses below slow RSI from overbought
            if rsi_fast < rsi_slow && rsi_fast > dynamic_thresholds.rsi_overbought - 10.0 && rsi_slow > dynamic_thresholds.rsi_overbought - 5.0 {
                signal_type = SignalType::Sell;
                confidence += 0.3;
                reasoning.push(format!("RSI divergence: Fast RSI ({:.2}) < Slow RSI ({:.2}) from overbought (threshold: {:.1})", 
                                     rsi_fast, rsi_slow, dynamic_thresholds.rsi_overbought));
            }
        }

        // Strategy 2: Moving Average Crossover
        if let (Some(sma_short), Some(sma_long)) = (short_term_indicators.sma_short, short_term_indicators.sma_long) {
            let ma_ratio = sma_short / sma_long;
            
            // Strong uptrend - use SMA50 for trend confirmation
            if ma_ratio > 1.02 && current_price > sma_long {
                if signal_type == SignalType::Buy {
                    confidence += 0.2;
                } else {
                    signal_type = SignalType::Buy;
                    confidence += 0.2;
                }
                reasoning.push(format!("Strong uptrend: SMA ratio {:.3}, price above SMA50", ma_ratio));
            }
            
            // Strong downtrend - use SMA50 for trend confirmation
            if ma_ratio < 0.98 && current_price < sma_long {
                if signal_type == SignalType::Sell {
                    confidence += 0.2;
                } else {
                    signal_type = SignalType::Sell;
                    confidence += 0.2;
                }
                reasoning.push(format!("Strong downtrend: SMA ratio {:.3}, price below SMA50", ma_ratio));
            }
        }

        // Strategy 3: Volatility Breakout (with dynamic thresholds)
        if let Some(volatility) = short_term_indicators.volatility {
            let avg_volatility = 0.02; // 2% average volatility
            let volatility_threshold = avg_volatility * dynamic_thresholds.volatility_multiplier;
            
            if volatility > volatility_threshold {
                // High volatility - look for momentum continuation
                if short_term_indicators.price_momentum.unwrap_or(0.0) > dynamic_thresholds.momentum_threshold {
                    if signal_type == SignalType::Buy {
                        confidence += 0.15;
                    } else {
                        signal_type = SignalType::Buy;
                        confidence += 0.15;
                    }
                    reasoning.push(format!("Volatility breakout: {:.3}% volatility (threshold: {:.3}%) with positive momentum", 
                                         volatility * 100.0, volatility_threshold * 100.0));
                } else if short_term_indicators.price_momentum.unwrap_or(0.0) < -dynamic_thresholds.momentum_threshold {
                    if signal_type == SignalType::Sell {
                        confidence += 0.15;
                    } else {
                        signal_type = SignalType::Sell;
                        confidence += 0.15;
                    }
                    reasoning.push(format!("Volatility breakout: {:.3}% volatility (threshold: {:.3}%) with negative momentum", 
                                         volatility * 100.0, volatility_threshold * 100.0));
                }
            }
        }

        // Strategy 4: Mean Reversion (with dynamic thresholds)
        if let Some(rsi_fast) = short_term_indicators.rsi_fast {
            if rsi_fast < dynamic_thresholds.rsi_oversold - 10.0 && signal_type == SignalType::Hold {
                signal_type = SignalType::Buy;
                confidence += 0.25;
                reasoning.push(format!("Mean reversion: Extreme oversold RSI ({:.2}) < {:.1}", 
                                     rsi_fast, dynamic_thresholds.rsi_oversold - 10.0));
            } else if rsi_fast > dynamic_thresholds.rsi_overbought + 10.0 && signal_type == SignalType::Hold {
                signal_type = SignalType::Sell;
                confidence += 0.25;
                reasoning.push(format!("Mean reversion: Extreme overbought RSI ({:.2}) > {:.1}", 
                                     rsi_fast, dynamic_thresholds.rsi_overbought + 10.0));
            }
        }

        // Strategy 5: Simple RSI Overbought/Oversold (should trigger more easily)
        if let Some(rsi_fast) = short_term_indicators.rsi_fast {
            if rsi_fast > dynamic_thresholds.rsi_overbought && signal_type == SignalType::Hold {
                signal_type = SignalType::Sell;
                confidence += 0.35;
                reasoning.push(format!("RSI overbought: RSI ({:.2}) > {:.1}", 
                                     rsi_fast, dynamic_thresholds.rsi_overbought));
            } else if rsi_fast < dynamic_thresholds.rsi_oversold && signal_type == SignalType::Hold {
                signal_type = SignalType::Buy;
                confidence += 0.35;
                reasoning.push(format!("RSI oversold: RSI ({:.2}) < {:.1}", 
                                     rsi_fast, dynamic_thresholds.rsi_oversold));
            }
        }

        // Strategy 5.5: Profit Taking with Momentum Weakening (NEW)
        if let Some(rsi_fast) = short_term_indicators.rsi_fast {
            // Sell when RSI is approaching overbought (60-70) and momentum is weakening
            if rsi_fast > 60.0 && rsi_fast < dynamic_thresholds.rsi_overbought && 
               short_term_indicators.price_momentum.unwrap_or(0.0) < 0.0 && signal_type == SignalType::Hold {
                signal_type = SignalType::Sell;
                confidence += 0.25;
                reasoning.push(format!("Profit taking: RSI ({:.2}) approaching overbought with weakening momentum", rsi_fast));
            }
        }

        // Strategy 6: Price Momentum Confirmation (with dynamic thresholds)
        if short_term_indicators.price_change_percent.abs() > dynamic_thresholds.momentum_threshold {
            if short_term_indicators.price_change_percent > dynamic_thresholds.momentum_threshold && signal_type == SignalType::Buy {
                confidence += 0.1;
                reasoning.push(format!("Momentum confirmation: {:.2}% price increase (threshold: {:.2}%)", 
                                     short_term_indicators.price_change_percent * 100.0, dynamic_thresholds.momentum_threshold * 100.0));
            } else if short_term_indicators.price_change_percent < -dynamic_thresholds.momentum_threshold && signal_type == SignalType::Sell {
                confidence += 0.1;
                reasoning.push(format!("Momentum confirmation: {:.2}% price decrease (threshold: {:.2}%)", 
                                     short_term_indicators.price_change_percent * 100.0, dynamic_thresholds.momentum_threshold * 100.0));
            }
        }

        // Strategy 7: Enhanced Trend Following with Market Regime
        if let (Some(sma_short), Some(rsi_fast)) = (short_term_indicators.sma_short, short_term_indicators.rsi_fast) {
            let base_confidence = match dynamic_thresholds.market_regime {
                MarketRegime::Trending => 0.35, // Higher confidence in trending markets
                MarketRegime::Ranging => 0.20,  // Lower confidence in ranging markets
                MarketRegime::Volatile => 0.30, // Medium confidence in volatile markets
                MarketRegime::Consolidating => 0.15, // Low confidence in consolidation
            };
            
            // Bullish trend: Price above SMA and RSI in bullish territory
            if current_price > sma_short && rsi_fast >= 40.0 && rsi_fast <= 70.0 {
                info!("[Trend Debug] Bullish: price {:.4} > SMA {:.4}, RSI {:.2}, Regime: {:?}", 
                      current_price, sma_short, rsi_fast, dynamic_thresholds.market_regime);
                
                let adjusted_confidence = base_confidence * (1.0 + dynamic_thresholds.trend_strength);
                
                if signal_type == SignalType::Buy {
                    confidence += adjusted_confidence;
                } else {
                    signal_type = SignalType::Buy;
                    confidence += adjusted_confidence;
                }
                reasoning.push(format!("Enhanced trend following: Price (${:.4}) above SMA (${:.4}), RSI ({:.2}) in bullish range, Regime: {:?}, Trend Strength: {:.2}", 
                                     current_price, sma_short, rsi_fast, dynamic_thresholds.market_regime, dynamic_thresholds.trend_strength));
            }
            
            // Bearish trend: Price below SMA and RSI in bearish territory
            if current_price < sma_short && rsi_fast >= 30.0 && rsi_fast <= 60.0 {
                info!("[Trend Debug] Bearish: price {:.4} < SMA {:.4}, RSI {:.2}, Regime: {:?}", 
                      current_price, sma_short, rsi_fast, dynamic_thresholds.market_regime);
                
                let adjusted_confidence = base_confidence * (1.0 + dynamic_thresholds.trend_strength);
                
                if signal_type == SignalType::Sell {
                    confidence += adjusted_confidence;
                } else {
                    signal_type = SignalType::Sell;
                    confidence += adjusted_confidence;
                }
                reasoning.push(format!("Enhanced trend following: Price (${:.4}) below SMA (${:.4}), RSI ({:.2}) in bearish range, Regime: {:?}, Trend Strength: {:.2}", 
                                     current_price, sma_short, rsi_fast, dynamic_thresholds.market_regime, dynamic_thresholds.trend_strength));
            }
        }

        // Strategy 8: Support/Resistance Breakout
        if let (Some(support), Some(resistance)) = (dynamic_thresholds.support_level, dynamic_thresholds.resistance_level) {
            let breakout_threshold = 0.002; // 0.2% breakout threshold
            
            // Breakout above resistance
            if current_price > resistance * (1.0 + breakout_threshold) && signal_type == SignalType::Hold {
                signal_type = SignalType::Buy;
                confidence += 0.30;
                reasoning.push(format!("Resistance breakout: Price (${:.4}) above resistance (${:.4})", 
                                     current_price, resistance));
            }
            
            // Breakdown below support
            if current_price < support * (1.0 - breakout_threshold) && signal_type == SignalType::Hold {
                signal_type = SignalType::Sell;
                confidence += 0.30;
                reasoning.push(format!("Support breakdown: Price (${:.4}) below support (${:.4})", 
                                     current_price, support));
            }
        }

        // Strategy 8.5: Trend Reversal Profit Taking (NEW)
        if let (Some(sma_short), Some(rsi_fast)) = (short_term_indicators.sma_short, short_term_indicators.rsi_fast) {
            // Sell when price is above SMA but momentum is weakening and RSI is high
            if current_price > sma_short && rsi_fast > 55.0 && 
               short_term_indicators.price_momentum.unwrap_or(0.0) < -dynamic_thresholds.momentum_threshold && 
               signal_type == SignalType::Hold {
                signal_type = SignalType::Sell;
                confidence += 0.20;
                reasoning.push(format!("Trend reversal profit taking: Price above SMA but momentum weakening, RSI {:.2}", rsi_fast));
            }
        }

        // Strategy 9: Mean Reversion at Support/Resistance
        if let (Some(support), Some(resistance)) = (dynamic_thresholds.support_level, dynamic_thresholds.resistance_level) {
            let mean_reversion_threshold = 0.005; // 0.5% from level
            
            // Bounce from support
            if current_price > support && current_price < support * (1.0 + mean_reversion_threshold) && 
               short_term_indicators.price_change_percent > 0.0 && signal_type == SignalType::Hold {
                signal_type = SignalType::Buy;
                confidence += 0.25;
                reasoning.push(format!("Support bounce: Price (${:.4}) near support (${:.4}) with positive momentum", 
                                     current_price, support));
            }
            
            // Rejection from resistance
            if current_price < resistance && current_price > resistance * (1.0 - mean_reversion_threshold) && 
               short_term_indicators.price_change_percent < 0.0 && signal_type == SignalType::Hold {
                signal_type = SignalType::Sell;
                confidence += 0.25;
                reasoning.push(format!("Resistance rejection: Price (${:.4}) near resistance (${:.4}) with negative momentum", 
                                     current_price, resistance));
            }
        }

        // Strategy 10: Multi-timeframe Trend Alignment
        if let (Some(short_sma), Some(medium_sma), Some(long_sma)) = 
            (short_term_indicators.sma_short, medium_term_indicators.sma_short, long_term_indicators.sma_short) {
            
            // All timeframes showing bullish alignment
            if current_price > short_sma && short_sma > medium_sma && medium_sma > long_sma {
                if signal_type == SignalType::Buy {
                    confidence += 0.20;
                } else {
                    signal_type = SignalType::Buy;
                    confidence += 0.20;
                }
                reasoning.push(format!("Multi-timeframe bullish alignment: Price > Short SMA ({:.4}) > Medium SMA ({:.4}) > Long SMA ({:.4})", 
                                     short_sma, medium_sma, long_sma));
            }
            
            // All timeframes showing bearish alignment
            if current_price < short_sma && short_sma < medium_sma && medium_sma < long_sma {
                if signal_type == SignalType::Sell {
                    confidence += 0.20;
                } else {
                    signal_type = SignalType::Sell;
                    confidence += 0.20;
                }
                reasoning.push(format!("Multi-timeframe bearish alignment: Price < Short SMA ({:.4}) < Medium SMA ({:.4}) < Long SMA ({:.4})", 
                                     short_sma, medium_sma, long_sma));
            }
        }

        // Strategy 11: RSI Divergence Across Timeframes
        if let (Some(short_rsi), Some(medium_rsi), Some(long_rsi)) = 
            (short_term_indicators.rsi_fast, medium_term_indicators.rsi_fast, long_term_indicators.rsi_fast) {
            
            // Bullish divergence: Short-term RSI rising while longer-term RSI is still low
            if short_rsi > medium_rsi && medium_rsi < 40.0 && short_rsi > 30.0 && signal_type == SignalType::Hold {
                signal_type = SignalType::Buy;
                confidence += 0.30;
                reasoning.push(format!("Multi-timeframe RSI bullish divergence: Short-term RSI ({:.2}) > Medium-term RSI ({:.2}) from oversold", 
                                     short_rsi, medium_rsi));
            }
            
            // Bearish divergence: Short-term RSI falling while longer-term RSI is still high
            if short_rsi < medium_rsi && medium_rsi > 60.0 && short_rsi < 70.0 && signal_type == SignalType::Hold {
                signal_type = SignalType::Sell;
                confidence += 0.30;
                reasoning.push(format!("Multi-timeframe RSI bearish divergence: Short-term RSI ({:.2}) < Medium-term RSI ({:.2}) from overbought", 
                                     short_rsi, medium_rsi));
            }
        }

        // Cap confidence at 1.0
        confidence = confidence.min(1.0_f64);

        // Only generate signals if confidence is high enough (increased threshold for more conservative trading)
        if confidence < 0.45 {
            signal_type = SignalType::Hold;
            reasoning.push("Insufficient confidence for trade signal".to_string());
        }

        TradingSignal {
            signal_type,
            price: current_price,
            timestamp,
            confidence,
            reasoning,
            take_profit: dynamic_thresholds.take_profit,
            stop_loss: dynamic_thresholds.stop_loss,
        }
    }

    fn analyze_market_regime(&self, prices: &[PriceFeed], indicators: &TradingIndicators) -> (MarketRegime, f64) {
        if prices.len() < 50 {
            return (MarketRegime::Consolidating, 0.0);
        }

        let price_values: Vec<f64> = prices.iter().map(|p| p.price).collect();
        
        // Calculate trend strength using linear regression
        let trend_strength = self.calculate_trend_strength(&price_values);
        
        // Calculate price range and volatility
        let min_price = price_values.iter().fold(f64::INFINITY, |a, &b| a.min(b));
        let max_price = price_values.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b));
        let price_range = (max_price - min_price) / min_price;
        let volatility = indicators.volatility.unwrap_or(0.02);
        
        // Determine market regime
        let regime = if trend_strength > 0.7 && price_range > 0.1 {
            MarketRegime::Trending
        } else if volatility > 0.05 {
            MarketRegime::Volatile
        } else if price_range < 0.05 {
            MarketRegime::Consolidating
        } else {
            MarketRegime::Ranging
        };
        
        (regime, trend_strength)
    }

    fn calculate_trend_strength(&self, prices: &[f64]) -> f64 {
        if prices.len() < 20 {
            return 0.0;
        }
        
        // Use linear regression to measure trend strength
        let n = prices.len() as f64;
        let x_values: Vec<f64> = (0..prices.len()).map(|i| i as f64).collect();
        
        let sum_x: f64 = x_values.iter().sum();
        let sum_y: f64 = prices.iter().sum();
        let sum_xy: f64 = x_values.iter().zip(prices.iter()).map(|(x, y)| x * y).sum();
        let sum_x2: f64 = x_values.iter().map(|x| x * x).sum();
        
        let slope = (n * sum_xy - sum_x * sum_y) / (n * sum_x2 - sum_x * sum_x);
        let mean_price = sum_y / n;
        
        // Normalize slope to get trend strength (0-1)
        let trend_strength = (slope / mean_price).abs().min(1.0);
        
        trend_strength
    }

    fn calculate_support_resistance(&self, prices: &[PriceFeed]) -> (Option<f64>, Option<f64>) {
        if prices.len() < 20 {
            return (None, None);
        }
        
        let price_values: Vec<f64> = prices.iter().map(|p| p.price).collect();
        
        // Find local minima and maxima
        let mut support_levels = Vec::new();
        let mut resistance_levels = Vec::new();
        
        for i in 2..price_values.len()-2 {
            let current = price_values[i];
            let prev1 = price_values[i-1];
            let prev2 = price_values[i-2];
            let next1 = price_values[i+1];
            let next2 = price_values[i+2];
            
            // Local minimum (support)
            if current < prev1 && current < prev2 && current < next1 && current < next2 {
                support_levels.push(current);
            }
            
            // Local maximum (resistance)
            if current > prev1 && current > prev2 && current > next1 && current > next2 {
                resistance_levels.push(current);
            }
        }
        
        // Get the most recent support and resistance levels
        let support = support_levels.last().copied();
        let resistance = resistance_levels.last().copied();
        
        (support, resistance)
    }

    fn get_recent_prices(&self, prices: &[PriceFeed], seconds_back: u64) -> Vec<PriceFeed> {
        if prices.is_empty() {
            return Vec::new();
        }

        let cutoff_time = prices.last().unwrap().timestamp - chrono::Duration::seconds(seconds_back as i64);
        
        prices.iter()
            .filter(|price| price.timestamp >= cutoff_time)
            .cloned()
            .collect()
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