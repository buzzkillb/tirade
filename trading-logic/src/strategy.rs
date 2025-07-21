use crate::models::{TradingSignal, SignalType, TradingIndicators, PriceFeed, BollingerBands, MACD, ExponentialSmoothing};
use crate::config::Config;
use chrono::{DateTime, Utc};
use tracing::info;

// Removed local struct definitions for BollingerBands, MACD, ExponentialSmoothing

pub struct TradingStrategy {
    config: Config,
    last_signal: Option<TradingSignal>,
}

#[derive(Debug, Clone)]
pub struct DynamicThresholds {
    pub rsi_oversold: f64,
    pub rsi_overbought: f64,
    pub momentum_threshold: f64,
    pub volatility_multiplier: f64,
    pub market_regime: MarketRegime,
    pub trend_strength: f64,
    pub support_level: Option<f64>,
    pub resistance_level: Option<f64>,
}

#[derive(Debug, Clone, PartialEq)]
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
        
        // Base thresholds (adaptive based on market regime)
        let (base_rsi_oversold, base_rsi_overbought, base_momentum_threshold) = 
            match market_regime {
                MarketRegime::Trending => (30.0, 70.0, 0.003),
                MarketRegime::Ranging => (35.0, 65.0, 0.004),
                MarketRegime::Volatile => (25.0, 75.0, 0.006),
                MarketRegime::Consolidating => (40.0, 60.0, 0.002),
            };
        
        // Adjust thresholds based on volatility
        let volatility_multiplier = (volatility / 0.02).max(0.5).min(2.0);
        let rsi_oversold = base_rsi_oversold * (1.0 + (1.0 - volatility_multiplier) * 0.1);
        let rsi_overbought = base_rsi_overbought * (1.0 + (volatility_multiplier - 1.0) * 0.1);
        let momentum_threshold = base_momentum_threshold * volatility_multiplier;
        
        info!("🎯 Dynamic Thresholds - Regime: {:?}, Trend Strength: {:.2}, Volatility: {:.2}%, RSI: {:.1}-{:.1}, Momentum: {:.3}",
              market_regime, trend_strength, volatility * 100.0, rsi_oversold, rsi_overbought, momentum_threshold);
        
        DynamicThresholds {
            rsi_oversold,
            rsi_overbought,
            momentum_threshold,
            volatility_multiplier,
            market_regime,
            trend_strength,
            support_level,
            resistance_level,
        }
    }
    
    // Detect momentum decay for early exit
    pub fn detect_momentum_decay(&self, prices: &[PriceFeed]) -> bool {
        if prices.len() < 10 {
            return false;
        }
        
        // Calculate recent momentum (last 5 periods)
        let recent_prices: Vec<f64> = prices.iter().rev().take(5).map(|p| p.price).collect();
        let recent_momentum = self.calculate_price_momentum(&recent_prices).unwrap_or(0.0);
        
        // Calculate earlier momentum (periods 6-10)
        let earlier_prices: Vec<f64> = prices.iter().rev().skip(5).take(5).map(|p| p.price).collect();
        let earlier_momentum = self.calculate_price_momentum(&earlier_prices).unwrap_or(0.0);
        
        // Return true if momentum is declining significantly
        recent_momentum < earlier_momentum * 0.7
    }
    
    // Check for RSI divergence exit conditions
    pub fn should_exit_rsi_divergence(&self, rsi: f64, price_momentum: f64, pnl: f64) -> bool {
        // Exit if RSI is weakening while in profit
        rsi > 60.0 && rsi < 70.0 && price_momentum < 0.0 && pnl > 0.003
    }

    pub fn calculate_custom_indicators(&self, prices: &[PriceFeed]) -> TradingIndicators {
        if prices.len() < self.config.sma_long_period {
            return TradingIndicators {
                rsi_fast: None,
                rsi_slow: None,
                sma_short: None,
                sma_long: None,
                volatility: None,
                price_momentum: None,
                price_change_percent: 0.0,
                bollinger_bands: None,
                macd: None,
                exponential_smoothing: None,
                stochastic: None,
                rsi_divergence: None,
                confluence_score: None,
            };
        }

        let price_values: Vec<f64> = prices.iter().map(|p| p.price).collect();
        let rsi_fast = self.calculate_rsi(&price_values, self.config.rsi_fast_period);
        let rsi_slow = self.calculate_rsi(&price_values, self.config.rsi_slow_period);
        let sma_short = self.calculate_sma(&price_values, self.config.sma_short_period);
        let sma_long = self.calculate_sma(&price_values, self.config.sma_long_period);
        let volatility = self.calculate_volatility(&price_values, self.config.volatility_window);
        let price_momentum = self.calculate_price_momentum(&price_values);
        let price_change_percent = if prices.len() >= 2 {
            let current = prices[prices.len() - 1].price;
            let previous = prices[prices.len() - 2].price;
            (current - previous) / previous
        } else {
            0.0
        };
        let bollinger_bands = self.calculate_bollinger_bands(&price_values, 20, 2.0);
        let macd = self.calculate_macd(&price_values, 12, 26, 9);
        let exponential_smoothing = self.calculate_exponential_smoothing(&price_values);
        let stochastic = self.calculate_stochastic(&price_values, 14, 3);
        let rsi_divergence = self.calculate_rsi_divergence(&price_values, 14);
        let confluence_score = self.calculate_confluence_score(
            &rsi_fast, &bollinger_bands, &macd, &stochastic, &rsi_divergence
        );

        TradingIndicators {
            rsi_fast,
            rsi_slow,
            sma_short,
            sma_long,
            volatility,
            price_momentum,
            price_change_percent,
            bollinger_bands,
            macd,
            exponential_smoothing,
            stochastic,
            rsi_divergence,
            confluence_score,
        }
    }

    fn generate_signal(
        &self,
        current_price: f64,
        timestamp: DateTime<Utc>,
        short_term_indicators: &TradingIndicators,
        _medium_term_indicators: &TradingIndicators,
        _long_term_indicators: &TradingIndicators,
        _db_indicators: &crate::models::TechnicalIndicators,
        dynamic_thresholds: &DynamicThresholds,
    ) -> TradingSignal {
        let mut confidence: f64 = 0.0;
        let mut reasoning = Vec::new();
        let mut signal_type = SignalType::Hold;

        // === Option 2: Only RSI and Moving Average Trend ===
        // 1. RSI Overbought/Oversold with Multi-Timeframe Confirmation
        if let Some(rsi_fast) = short_term_indicators.rsi_fast {
            info!("🔍 RSI Analysis: Current RSI {:.2}, Oversold threshold {:.1}, Overbought threshold {:.1}", 
                  rsi_fast, dynamic_thresholds.rsi_oversold, dynamic_thresholds.rsi_overbought);
            
            // Get medium-term RSI for confirmation
            let medium_rsi = _medium_term_indicators.rsi_fast.unwrap_or(50.0);
            let long_rsi = _long_term_indicators.rsi_fast.unwrap_or(50.0);
            
            if rsi_fast > dynamic_thresholds.rsi_overbought {
                // Multi-timeframe confirmation for SELL
                let medium_confirms = medium_rsi > (dynamic_thresholds.rsi_overbought - 10.0); // Medium RSI > 60
                let long_confirms = long_rsi > 50.0; // Long RSI above neutral
                
                if medium_confirms && long_confirms {
                    signal_type = SignalType::Sell;
                    confidence += 0.5;
                    reasoning.push(format!("Multi-timeframe RSI overbought: Short ({:.1}) > {:.1}, Medium ({:.1}) confirms, Long ({:.1}) neutral+", 
                                         rsi_fast, dynamic_thresholds.rsi_overbought, medium_rsi, long_rsi));
                    info!("🎯 RSI SELL: Multi-timeframe confirmed ({:.1}/{:.1}/{:.1})", rsi_fast, medium_rsi, long_rsi);
                } else {
                    info!("🚫 RSI SELL: Multi-timeframe conflict ({:.1}/{:.1}/{:.1})", rsi_fast, medium_rsi, long_rsi);
                }
            } else if rsi_fast < dynamic_thresholds.rsi_oversold {
                // Multi-timeframe confirmation for BUY
                let medium_confirms = medium_rsi < (dynamic_thresholds.rsi_oversold + 10.0); // Medium RSI < 50
                let long_confirms = long_rsi < 60.0; // Long RSI below overbought
                
                if medium_confirms && long_confirms {
                    signal_type = SignalType::Buy;
                    confidence += 0.5;
                    reasoning.push(format!("Multi-timeframe RSI oversold: Short ({:.1}) < {:.1}, Medium ({:.1}) confirms, Long ({:.1}) not overbought", 
                                         rsi_fast, dynamic_thresholds.rsi_oversold, medium_rsi, long_rsi));
                    info!("🎯 RSI BUY: Multi-timeframe confirmed ({:.1}/{:.1}/{:.1})", rsi_fast, medium_rsi, long_rsi);
                } else {
                    info!("🚫 RSI BUY: Multi-timeframe conflict ({:.1}/{:.1}/{:.1})", rsi_fast, medium_rsi, long_rsi);
                }
            }
        }
        
        // 2. Moving Average Trend (only if not already set by RSI)
        if let (Some(sma_short), Some(rsi_fast)) = (short_term_indicators.sma_short, short_term_indicators.rsi_fast) {
            if signal_type == SignalType::Hold {
                // Uptrend: Price above SMA and RSI neutral (40-60)
                if current_price > sma_short && rsi_fast >= 40.0 && rsi_fast <= 60.0 {
                    signal_type = SignalType::Buy;
                    confidence += 0.3;
                    reasoning.push(format!("MA trend: Price (${:.4}) above SMA (${:.4}), RSI ({:.2}) neutral (40-60)", current_price, sma_short, rsi_fast));
                    info!("🎯 MA BUY: Price above SMA, RSI neutral");
                }
                // Downtrend: Price below SMA and RSI neutral (40-60)
                else if current_price < sma_short && rsi_fast >= 40.0 && rsi_fast <= 60.0 {
                    signal_type = SignalType::Sell;
                    confidence += 0.3;
                    reasoning.push(format!("MA trend: Price (${:.4}) below SMA (${:.4}), RSI ({:.2}) neutral (40-60)", current_price, sma_short, rsi_fast));
                    info!("🎯 MA SELL: Price below SMA, RSI neutral");
                }
            }
        }

        // === All other strategies are commented out for Option 2 simplification ===
        /*
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
            
            // FIXED: Price below SMA should generate BUY signal (buying the dip)
            if current_price < sma_short && rsi_fast >= 30.0 && rsi_fast <= 60.0 {
                let adjusted_confidence = base_confidence * (1.0 + dynamic_thresholds.trend_strength);
                
                if signal_type == SignalType::Buy {
                    confidence += adjusted_confidence;
                } else {
                    signal_type = SignalType::Buy; // FIXED: BUY instead of SELL
                    confidence += adjusted_confidence;
                }
                reasoning.push(format!("Enhanced trend following: Price (${:.4}) below SMA (${:.4}), RSI ({:.2}) in bearish range - BUYING THE DIP, Regime: {:?}, Trend Strength: {:.2}", 
                                     current_price, sma_short, rsi_fast, dynamic_thresholds.market_regime, dynamic_thresholds.trend_strength));
            }
        }

        // Strategy 8: Support/Resistance Breakout
        if let (Some(support), Some(resistance)) = (dynamic_thresholds.support_level, dynamic_thresholds.resistance_level) {
            let support_distance = (current_price - support) / current_price;
            let resistance_distance = (resistance - current_price) / current_price;
            
            // Breakout above resistance
            if current_price > resistance * 1.005 && signal_type == SignalType::Hold {
                signal_type = SignalType::Buy;
                confidence += 0.3;
                reasoning.push(format!("Resistance breakout: Price (${:.4}) above resistance (${:.4})", current_price, resistance));
            }
            
            // Breakdown below support
            if current_price < support * 0.995 && signal_type == SignalType::Hold {
                signal_type = SignalType::Sell;
                confidence += 0.3;
                reasoning.push(format!("Support breakdown: Price (${:.4}) below support (${:.4})", current_price, support));
            }
        }

        // Strategy 9: Bollinger Bands Analysis (NEW)
        if let Some(bollinger) = &short_term_indicators.bollinger_bands {
            // Oversold with bullish momentum
            if bollinger.percent_b < 0.2 && short_term_indicators.price_momentum.unwrap_or(0.0) > 0.0 {
                if signal_type == SignalType::Buy {
                    confidence += 0.25;
                } else {
                    signal_type = SignalType::Buy;
                    confidence += 0.25;
                }
                reasoning.push(format!("Bollinger oversold: %B ({:.2}) < 0.2 with bullish momentum", bollinger.percent_b));
            }
            
            // Overbought with bearish momentum
            if bollinger.percent_b > 0.8 && short_term_indicators.price_momentum.unwrap_or(0.0) < 0.0 {
                if signal_type == SignalType::Sell {
                    confidence += 0.25;
                } else {
                    signal_type = SignalType::Sell;
                    confidence += 0.25;
                }
                reasoning.push(format!("Bollinger overbought: %B ({:.2}) > 0.8 with bearish momentum", bollinger.percent_b));
            }
        }

        // Strategy 10: MACD Analysis (NEW)
        if let Some(macd) = &short_term_indicators.macd {
            // Bullish crossover
            if macd.bullish_crossover && signal_type == SignalType::Hold {
                signal_type = SignalType::Buy;
                confidence += 0.3;
                reasoning.push(format!("MACD bullish crossover: MACD ({:.4}) > Signal ({:.4})", macd.macd_line, macd.signal_line));
            }
            
            // Bearish crossover
            if macd.bearish_crossover && signal_type == SignalType::Hold {
                signal_type = SignalType::Sell;
                confidence += 0.3;
                reasoning.push(format!("MACD bearish crossover: MACD ({:.4}) < Signal ({:.4})", macd.macd_line, macd.signal_line));
            }
            
            // MACD histogram momentum
            if macd.histogram > 0.0 && macd.histogram > macd.signal_line * 0.1 && signal_type == SignalType::Buy {
                confidence += 0.15;
                reasoning.push(format!("MACD momentum: Histogram ({:.4}) showing strong bullish momentum", macd.histogram));
            } else if macd.histogram < 0.0 && macd.histogram < macd.signal_line * -0.1 && signal_type == SignalType::Sell {
                confidence += 0.15;
                reasoning.push(format!("MACD momentum: Histogram ({:.4}) showing strong bearish momentum", macd.histogram));
            }
        }

        // Strategy 11: EMA Analysis (NEW)
        if let Some(ema) = &short_term_indicators.exponential_smoothing {
            // Strong uptrend: EMA12 > EMA26 > EMA50
            if ema.ema_12 > ema.ema_26 && ema.ema_26 > ema.ema_50 && current_price > ema.ema_12 {
                if signal_type == SignalType::Buy {
                    confidence += 0.2;
                } else {
                    signal_type = SignalType::Buy;
                    confidence += 0.2;
                }
                reasoning.push(format!("EMA strong uptrend: EMA12 ({:.4}) > EMA26 ({:.4}) > EMA50 ({:.4})", ema.ema_12, ema.ema_26, ema.ema_50));
            }
            
            // Strong downtrend: EMA12 < EMA26 < EMA50
            if ema.ema_12 < ema.ema_26 && ema.ema_26 < ema.ema_50 && current_price < ema.ema_12 {
                if signal_type == SignalType::Sell {
                    confidence += 0.2;
                } else {
                    signal_type = SignalType::Sell;
                    confidence += 0.2;
                }
                reasoning.push(format!("EMA strong downtrend: EMA12 ({:.4}) < EMA26 ({:.4}) < EMA50 ({:.4})", ema.ema_12, ema.ema_26, ema.ema_50));
            }
        }

        // Strategy 12: Stochastic Oscillator Analysis (NEW)
        if let Some(stochastic) = &short_term_indicators.stochastic {
            // Oversold with bullish crossover
            if stochastic.oversold && stochastic.k > stochastic.d && signal_type == SignalType::Hold {
                signal_type = SignalType::Buy;
                confidence += 0.25;
                reasoning.push(format!("Stochastic oversold: K ({:.2}) > D ({:.2}) from oversold", stochastic.k, stochastic.d));
            }
            
            // Overbought with bearish crossover
            if stochastic.overbought && stochastic.k < stochastic.d && signal_type == SignalType::Hold {
                signal_type = SignalType::Sell;
                confidence += 0.25;
                reasoning.push(format!("Stochastic overbought: K ({:.2}) < D ({:.2}) from overbought", stochastic.k, stochastic.d));
            }
        }

        // Strategy 13: RSI Divergence Analysis (NEW)
        if let Some(rsi_divergence) = short_term_indicators.rsi_divergence {
            if rsi_divergence > 0.0 && signal_type == SignalType::Hold {
                signal_type = SignalType::Buy;
                confidence += 0.3;
                reasoning.push(format!("RSI bullish divergence: Divergence score ({:.2})", rsi_divergence));
            } else if rsi_divergence < 0.0 && signal_type == SignalType::Hold {
                signal_type = SignalType::Sell;
                confidence += 0.3;
                reasoning.push(format!("RSI bearish divergence: Divergence score ({:.2})", rsi_divergence));
            }
        }

        // Strategy 14: Confluence Score Boost (NEW)
        if let Some(confluence_score) = short_term_indicators.confluence_score {
            if confluence_score > 0.6 {
                confidence += confluence_score * 0.2; // Boost confidence by up to 20% based on confluence
                reasoning.push(format!("High confluence score: {:.2} - multiple indicators aligned", confluence_score));
            }
        }

        // Strategy 15: Multi-timeframe RSI Divergence
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
        */

        // Cap confidence at 1.0
        confidence = confidence.min(1.0_f64);

        // Check minimum volatility requirement to avoid micro-trading
        let min_volatility_threshold = std::env::var("MIN_VOLATILITY_FOR_TRADING")
            .unwrap_or_else(|_| "0.0001".to_string())  // 0.01% minimum volatility (lowered from 0.05%)
            .parse::<f64>()
            .unwrap_or(0.0001);
        
        if let Some(current_volatility) = short_term_indicators.volatility {
            if current_volatility < min_volatility_threshold && signal_type != SignalType::Hold {
                signal_type = SignalType::Hold;
                reasoning.push(format!("Insufficient volatility for trading ({:.3}% < {:.3}%) - avoiding micro-trades", 
                             current_volatility * 100.0, min_volatility_threshold * 100.0));
                info!("🚫 Volatility too low: {:.3}% < {:.3}%", 
                      current_volatility * 100.0, min_volatility_threshold * 100.0);
            }
        }

        // Check minimum profit target to ensure trades cover fees
        let min_profit_target = std::env::var("MIN_PROFIT_TARGET")
            .unwrap_or_else(|_| "0.005".to_string())  // 0.5% minimum profit target
            .parse::<f64>()
            .unwrap_or(0.005);
        
        // Estimate potential profit based on recent price movement using momentum
        if signal_type != SignalType::Hold {
            if let Some(price_momentum) = short_term_indicators.price_momentum {
                let recent_price_change = price_momentum.abs();
                
                if recent_price_change < min_profit_target {
                    signal_type = SignalType::Hold;
                    reasoning.push(format!("Insufficient price movement for profitable trade ({:.3}% < {:.3}%) - avoiding fee erosion", 
                                 recent_price_change * 100.0, min_profit_target * 100.0));
                    info!("🚫 Profit too low: {:.3}% < {:.3}%", 
                          recent_price_change * 100.0, min_profit_target * 100.0);
                }
            }
        }

        // Only generate signals if confidence is high enough (using configurable threshold)
        if confidence < self.config.min_confidence_threshold {
            signal_type = SignalType::Hold;
            reasoning.push(format!("Insufficient confidence for trade signal ({}% < {}%)", 
                                 (confidence * 100.0) as i32, 
                                 (self.config.min_confidence_threshold * 100.0) as i32));
            info!("🚫 Signal: {:?} {:.1}% < {:.1}% threshold", signal_type, confidence * 100.0, self.config.min_confidence_threshold * 100.0);
        } else {
            info!("✅ Signal: {:?} {:.1}%", signal_type, confidence * 100.0);
        }

        TradingSignal {
            signal_type,
            price: current_price,
            timestamp,
            confidence,
            reasoning,
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

    pub fn calculate_rsi(&self, prices: &[f64], period: usize) -> Option<f64> {
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

    pub fn calculate_price_momentum(&self, prices: &[f64]) -> Option<f64> {
        if prices.len() < 2 {
            return None;
        }
        
        let current = prices[prices.len() - 1];
        let previous = prices[prices.len() - 2];
        Some((current - previous) / previous)
    }

    // New indicator calculations
    pub fn calculate_bollinger_bands(&self, prices: &[f64], period: usize, std_dev: f64) -> Option<BollingerBands> {
        if prices.len() < period {
            return None;
        }
        
        let sma = self.calculate_sma(prices, period)?;
        let variance = prices.iter()
            .skip(prices.len() - period)
            .map(|&price| (price - sma).powi(2))
            .sum::<f64>() / period as f64;
        let std = variance.sqrt();
        
        let upper = sma + (std_dev * std);
        let lower = sma - (std_dev * std);
        let bandwidth = (upper - lower) / sma;
        let percent_b = (prices.last().unwrap() - lower) / (upper - lower);
        let squeeze = bandwidth < 0.05; // Low volatility period
        
        Some(BollingerBands {
            upper,
            middle: sma,
            lower,
            bandwidth,
            percent_b,
            squeeze,
        })
    }

    pub fn calculate_macd(&self, prices: &[f64], fast_period: usize, slow_period: usize, signal_period: usize) -> Option<MACD> {
        if prices.len() < slow_period {
            return None;
        }
        
        let ema_fast = self.calculate_ema(prices, fast_period)?;
        let ema_slow = self.calculate_ema(prices, slow_period)?;
        let macd_line = ema_fast - ema_slow;
        
        // Calculate signal line (EMA of MACD line)
        let macd_values: Vec<f64> = prices.iter()
            .enumerate()
            .filter_map(|(i, _)| {
                if i >= slow_period - 1 {
                    let fast_ema = self.calculate_ema(&prices[..=i], fast_period)?;
                    let slow_ema = self.calculate_ema(&prices[..=i], slow_period)?;
                    Some(fast_ema - slow_ema)
                } else {
                    None
                }
            })
            .collect();
        
        let signal_line = if macd_values.len() >= signal_period {
            self.calculate_ema(&macd_values, signal_period)?
        } else {
            macd_values.last().copied().unwrap_or(0.0)
        };
        
        let histogram = macd_line - signal_line;
        
        // Detect crossovers (simplified - would need previous values for full implementation)
        let bullish_crossover = macd_line > signal_line && histogram > 0.0;
        let bearish_crossover = macd_line < signal_line && histogram < 0.0;
        
        Some(MACD {
            macd_line,
            signal_line,
            histogram,
            bullish_crossover,
            bearish_crossover,
        })
    }

    pub fn calculate_exponential_smoothing(&self, prices: &[f64]) -> Option<ExponentialSmoothing> {
        if prices.len() < 50 {
            return None;
        }
        
        let ema_12 = self.calculate_ema(prices, 12)?;
        let ema_26 = self.calculate_ema(prices, 26)?;
        let ema_50 = self.calculate_ema(prices, 50)?;
        
        // Use EMA-12 as smoothed price for noise reduction
        let smoothed_price = ema_12;
        
        Some(ExponentialSmoothing {
            ema_12,
            ema_26,
            ema_50,
            smoothed_price,
        })
    }

    fn calculate_ema(&self, prices: &[f64], period: usize) -> Option<f64> {
        if prices.len() < period {
            return None;
        }
        
        let alpha = 2.0 / (period as f64 + 1.0);
        let mut ema = prices[0];
        
        for &price in prices.iter().skip(1) {
            ema = alpha * price + (1.0 - alpha) * ema;
        }
        
        Some(ema)
    }

    pub fn calculate_stochastic(&self, prices: &[f64], k_period: usize, d_period: usize) -> Option<crate::models::StochasticOscillator> {
        if prices.len() < k_period + d_period {
            return None;
        }
        let mut k_values = Vec::new();
        for i in k_period..=prices.len() {
            let window = &prices[i - k_period..i];
            let high = window.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
            let low = window.iter().cloned().fold(f64::INFINITY, f64::min);
            let close = window[window.len() - 1];
            let k = if (high - low).abs() < 1e-8 { 0.0 } else { (close - low) / (high - low) };
            k_values.push(k);
        }
        let k = *k_values.last().unwrap_or(&0.0);
        let d = if k_values.len() >= d_period {
            k_values[k_values.len() - d_period..].iter().sum::<f64>() / d_period as f64
        } else {
            k
        };
        let overbought = k > 0.8;
        let oversold = k < 0.2;
        Some(crate::models::StochasticOscillator { k, d, overbought, oversold })
    }

    pub fn calculate_rsi_divergence(&self, prices: &[f64], period: usize) -> Option<f64> {
        if prices.len() < period * 2 {
            return None;
        }
        let rsi = self.calculate_rsi(prices, period)?;
        let prev_rsi = self.calculate_rsi(&prices[..prices.len() - period], period)?;
        let price_change = prices.last().unwrap() - prices[prices.len() - period - 1];
        let rsi_change = rsi - prev_rsi;
        // Divergence: price up, RSI down (bearish), or price down, RSI up (bullish)
        if price_change > 0.0 && rsi_change < 0.0 {
            Some(-1.0) // Bearish divergence
        } else if price_change < 0.0 && rsi_change > 0.0 {
            Some(1.0) // Bullish divergence
        } else {
            Some(0.0) // No divergence
        }
    }

    pub fn calculate_confluence_score(
        &self,
        rsi_fast: &Option<f64>,
        bollinger_bands: &Option<BollingerBands>,
        macd: &Option<MACD>,
        stochastic: &Option<crate::models::StochasticOscillator>,
        rsi_divergence: &Option<f64>,
    ) -> Option<f64> {
        let mut score = 0.0;
        if let Some(rsi) = rsi_fast {
            if *rsi > 60.0 || *rsi < 40.0 {
                score += 0.2;
            }
        }
        if let Some(bb) = bollinger_bands {
            if bb.percent_b < 0.2 || bb.percent_b > 0.8 {
                score += 0.2;
            }
        }
        if let Some(macd) = macd {
            if macd.bullish_crossover || macd.bearish_crossover {
                score += 0.2;
            }
        }
        if let Some(stoch) = stochastic {
            if stoch.overbought || stoch.oversold {
                score += 0.2;
            }
        }
        if let Some(div) = rsi_divergence {
            if *div != 0.0 {
                score += 0.2;
            }
        }
        Some(score)
    }
} 