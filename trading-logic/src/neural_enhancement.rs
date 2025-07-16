use crate::models::{TradingSignal, SignalType, PriceFeed, TradingIndicators};
use anyhow::{Result, anyhow};
use chrono::{DateTime, Utc};
use serde::{Serialize, Deserialize};
use std::collections::VecDeque;
use tracing::{info, warn, debug};

// Enhanced neural configuration
#[derive(Debug, Clone)]
pub struct NeuralConfig {
    pub enabled: bool,
    pub learning_rate: f64,
    pub memory_size: usize,
    pub confidence_threshold: f64,
    pub lstm_sequence_length: usize,
    pub momentum: f64,
    pub pattern_memory_size: usize,
    pub adaptation_rate: f64,
}

impl Default for NeuralConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            learning_rate: 0.01,
            memory_size: 100,
            confidence_threshold: 0.6,
            lstm_sequence_length: 20,
            momentum: 0.9,
            pattern_memory_size: 1000,
            adaptation_rate: 0.1,
        }
    }
}

// Enhanced neural predictions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NeuralPrediction {
    pub price_direction: f64,        // -1.0 to 1.0 (bearish to bullish)
    pub confidence: f64,             // 0.0 to 1.0
    pub pattern_strength: f64,       // 0.0 to 1.0
    pub risk_level: f64,            // 0.0 to 1.0 (low to high risk)
    pub volatility_forecast: f64,    // 0.0 to 1.0 (low to high volatility)
    pub market_regime: MarketRegime, // Current market regime
    pub optimal_position_size: f64,  // 0.0 to 1.0 (percentage of max position)
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum MarketRegime {
    Trending,
    Consolidating,
    Volatile,
    Breakout,
}

// Trade outcome for learning
#[derive(Debug, Clone)]
pub struct TradeOutcome {
    pub entry_price: f64,
    pub exit_price: f64,
    pub pnl: f64,
    pub success: bool,
    pub timestamp: DateTime<Utc>,
}

// Advanced online learning system with LSTM-like capabilities
#[derive(Debug, Clone)]
pub struct OnlineLearner {
    // Enhanced pattern recognition
    price_momentum_history: VecDeque<f64>,
    rsi_history: VecDeque<f64>,
    success_history: VecDeque<bool>,
    price_sequence: VecDeque<f64>,
    
    // LSTM-inspired weights and states
    momentum_weight: f64,
    rsi_weight: f64,
    volatility_weight: f64,
    pattern_weights: Vec<f64>,
    hidden_state: Vec<f64>,
    
    // Pattern memory for advanced learning
    pattern_memory: VecDeque<PatternMatch>,
    
    // Performance tracking
    total_predictions: u64,
    correct_predictions: u64,
    learning_rate: f64,
    sequence_length: usize,
}

#[derive(Debug, Clone)]
struct PatternMatch {
    pattern_features: Vec<f64>,
    confidence: f64,
    outcome: Option<f64>,
    timestamp: DateTime<Utc>,
}

impl OnlineLearner {
    pub fn new(config: &NeuralConfig) -> Self {
        Self {
            price_momentum_history: VecDeque::with_capacity(config.memory_size),
            rsi_history: VecDeque::with_capacity(config.memory_size),
            success_history: VecDeque::with_capacity(config.memory_size),
            price_sequence: VecDeque::with_capacity(config.lstm_sequence_length),
            momentum_weight: 0.3,
            rsi_weight: 0.4,
            volatility_weight: 0.3,
            pattern_weights: vec![0.5; 5], // Initialize 5 pattern filters
            hidden_state: vec![0.0; 10],   // Initialize hidden state
            pattern_memory: VecDeque::with_capacity(config.pattern_memory_size),
            total_predictions: 0,
            correct_predictions: 0,
            learning_rate: config.learning_rate,
            sequence_length: config.lstm_sequence_length,
        }
    }
    
    pub fn predict(&mut self, prices: &[PriceFeed], indicators: &TradingIndicators) -> Result<NeuralPrediction> {
        if prices.len() < 2 {
            return Ok(NeuralPrediction {
                price_direction: 0.0,
                confidence: 0.5,
                pattern_strength: 0.5,
                risk_level: 0.5,
                volatility_forecast: 0.5,
                market_regime: MarketRegime::Consolidating,
                optimal_position_size: 0.5,
                timestamp: Utc::now(),
            });
        }
        
        // Extract enhanced features
        let current_price = prices.last().unwrap().price;
        let features = self.extract_enhanced_features(prices, indicators)?;
        
        // Update price sequence for LSTM-like processing
        self.price_sequence.push_back(current_price);
        if self.price_sequence.len() > self.sequence_length {
            self.price_sequence.pop_front();
        }
        
        // Store for learning
        self.price_momentum_history.push_back(features[0]); // momentum
        self.rsi_history.push_back(features[1]); // normalized RSI
        
        // Keep memory size limited
        if self.price_momentum_history.len() > 50 {
            self.price_momentum_history.pop_front();
        }
        if self.rsi_history.len() > 50 {
            self.rsi_history.pop_front();
        }
        
        // Advanced neural network prediction with LSTM-like processing
        let price_direction = self.lstm_like_prediction(&features)?;
        
        // Enhanced pattern recognition
        let pattern_strength = self.advanced_pattern_recognition(prices)?;
        
        // Calculate confidence based on multiple factors
        let confidence = self.calculate_enhanced_confidence(price_direction, pattern_strength);
        
        // Risk level based on volatility and prediction uncertainty
        let volatility = features[2]; // volatility feature
        let risk_level = self.calculate_risk_level(volatility, confidence, pattern_strength);
        
        // Volatility forecast based on recent price movements
        let volatility_forecast = self.calculate_volatility_forecast(prices);
        
        // Market regime detection
        let market_regime = self.detect_market_regime(prices, volatility);
        
        // Optimal position size based on confidence and risk
        let optimal_position_size = self.calculate_optimal_position_size(confidence, risk_level);
        
        // Store pattern for learning
        self.store_pattern_for_learning(&features, confidence);
        
        self.total_predictions += 1;
        
        debug!("🧠 Enhanced Neural prediction: Direction {:.3}, Confidence {:.3}, Pattern {:.3}, Risk {:.3}, Regime {:?}", 
               price_direction, confidence, pattern_strength, risk_level, market_regime);
        
        Ok(NeuralPrediction {
            price_direction,
            confidence,
            pattern_strength,
            risk_level,
            volatility_forecast,
            market_regime,
            optimal_position_size,
            timestamp: Utc::now(),
        })
    }
    
    pub fn learn_from_outcome(&mut self, outcome: &TradeOutcome) -> Result<()> {
        let success = outcome.success;
        self.success_history.push_back(success);
        
        if self.success_history.len() > 20 {
            self.success_history.pop_front();
        }
        
        // Update accuracy tracking
        if success {
            self.correct_predictions += 1;
        }
        
        // Simple weight adjustment based on outcome
        let adjustment = if success { 
            self.learning_rate 
        } else { 
            -self.learning_rate * 0.5 
        };
        
        // Adjust weights based on which features were most influential
        if let (Some(&last_momentum), Some(&last_rsi)) = (
            self.price_momentum_history.back(),
            self.rsi_history.back()
        ) {
            if last_momentum.abs() > 0.01 {
                self.momentum_weight += adjustment * last_momentum.abs();
            }
            if (last_rsi - 0.5).abs() > 0.1 {
                self.rsi_weight += adjustment * (last_rsi - 0.5).abs();
            }
        }
        
        // Keep weights in reasonable bounds
        self.momentum_weight = self.momentum_weight.max(0.1).min(1.0);
        self.rsi_weight = self.rsi_weight.max(0.1).min(1.0);
        self.volatility_weight = self.volatility_weight.max(0.1).min(1.0);
        
        let accuracy = self.correct_predictions as f64 / self.total_predictions as f64;
        info!("🎓 Neural learning: PnL {:.2}%, Accuracy {:.1}%, Weights [M:{:.2}, R:{:.2}, V:{:.2}]", 
              outcome.pnl * 100.0, accuracy * 100.0, 
              self.momentum_weight, self.rsi_weight, self.volatility_weight);
        
        Ok(())
    }
    
    fn calculate_pattern_strength(&self) -> f64 {
        if self.price_momentum_history.len() < 5 {
            return 0.5;
        }
        
        // Calculate consistency of recent momentum
        let recent_momentum: Vec<f64> = self.price_momentum_history.iter()
            .rev().take(5).cloned().collect();
        
        let avg_momentum = recent_momentum.iter().sum::<f64>() / recent_momentum.len() as f64;
        let consistency = recent_momentum.iter()
            .map(|&m| (m - avg_momentum).abs())
            .sum::<f64>() / recent_momentum.len() as f64;
        
        // Lower consistency (less variation) = higher pattern strength
        (1.0 - consistency.min(1.0)).max(0.0)
    }
    
    pub fn get_accuracy(&self) -> f64 {
        if self.total_predictions > 0 {
            self.correct_predictions as f64 / self.total_predictions as f64
        } else {
            0.5
        }
    }
    
    pub fn get_learning_stats(&self) -> (u64, f64, f64, f64, f64) {
        (
            self.total_predictions,
            self.get_accuracy(),
            self.momentum_weight,
            self.rsi_weight,
            self.volatility_weight
        )
    }
    
    fn calculate_volatility_forecast(&self, prices: &[PriceFeed]) -> f64 {
        if prices.len() < 10 {
            return 0.5;
        }
        
        // Calculate recent volatility from price movements
        let price_values: Vec<f64> = prices.iter().map(|p| p.price).collect();
        let mut volatilities = Vec::new();
        
        for i in 1..price_values.len().min(20) {
            let return_rate = (price_values[i] - price_values[i-1]) / price_values[i-1];
            volatilities.push(return_rate.abs());
        }
        
        if volatilities.is_empty() {
            return 0.5;
        }
        
        let avg_volatility = volatilities.iter().sum::<f64>() / volatilities.len() as f64;
        avg_volatility.min(1.0)
    }
    
    fn detect_market_regime(&self, prices: &[PriceFeed], volatility: f64) -> MarketRegime {
        if prices.len() < 10 {
            return MarketRegime::Consolidating;
        }
        
        let price_values: Vec<f64> = prices.iter().map(|p| p.price).collect();
        
        // Calculate trend strength
        let trend_strength = self.calculate_trend_strength(&price_values);
        
        // Calculate momentum
        let momentum = self.calculate_momentum(&price_values);
        
        // Determine regime based on volatility, trend, and momentum
        if volatility > 0.05 {
            MarketRegime::Volatile
        } else if trend_strength.abs() > 0.6 {
            MarketRegime::Trending
        } else if momentum.abs() > 0.3 && volatility > 0.02 {
            MarketRegime::Breakout
        } else {
            MarketRegime::Consolidating
        }
    }
    
    fn calculate_trend_strength(&self, prices: &[f64]) -> f64 {
        if prices.len() < 10 {
            return 0.0;
        }
        
        let first_half = &prices[0..prices.len()/2];
        let second_half = &prices[prices.len()/2..];
        
        let first_avg = first_half.iter().sum::<f64>() / first_half.len() as f64;
        let second_avg = second_half.iter().sum::<f64>() / second_half.len() as f64;
        
        let trend = (second_avg - first_avg) / first_avg;
        trend.max(-1.0).min(1.0)
    }
    
    fn calculate_momentum(&self, prices: &[f64]) -> f64 {
        if prices.len() < 5 {
            return 0.0;
        }
        
        let recent_change = (prices[prices.len()-1] - prices[prices.len()-5]) / prices[prices.len()-5];
        recent_change.max(-1.0).min(1.0)
    }
    
    fn calculate_optimal_position_size(&self, confidence: f64, risk_level: f64) -> f64 {
        // Kelly criterion inspired position sizing
        let signal_strength = confidence;
        let risk_adjustment = 1.0 - risk_level;
        
        let optimal_size = (signal_strength * risk_adjustment * 0.5).max(0.1).min(1.0);
        optimal_size
    }
    
    // Advanced feature extraction for enhanced neural processing
    fn extract_enhanced_features(&self, prices: &[PriceFeed], indicators: &TradingIndicators) -> Result<Vec<f64>> {
        if prices.len() < 2 {
            return Ok(vec![0.0; 10]); // Return default features
        }
        
        let current_price = prices.last().unwrap().price;
        let previous_price = prices[prices.len() - 2].price;
        
        let mut features = Vec::new();
        
        // 1. Price momentum
        let momentum = (current_price - previous_price) / previous_price;
        features.push(momentum);
        
        // 2. Normalized RSI
        let rsi = indicators.rsi_fast.unwrap_or(50.0) / 100.0;
        features.push(rsi);
        
        // 3. Volatility
        let volatility = indicators.volatility.unwrap_or(0.01);
        features.push(volatility);
        
        // 4. SMA ratio (short/long)
        let sma_ratio = if let (Some(short), Some(long)) = (indicators.sma_short, indicators.sma_long) {
            if long > 0.0 { short / long } else { 1.0 }
        } else { 1.0 };
        features.push(sma_ratio);
        
        // 5. Price position relative to SMA
        let price_sma_ratio = if let Some(sma) = indicators.sma_short {
            if sma > 0.0 { current_price / sma } else { 1.0 }
        } else { 1.0 };
        features.push(price_sma_ratio);
        
        // 6-10. Recent price changes (last 5 periods)
        let price_values: Vec<f64> = prices.iter().map(|p| p.price).collect();
        for i in 1..6 {
            if price_values.len() > i {
                let idx = price_values.len() - 1 - i;
                if idx > 0 {
                    let change = (price_values[idx] - price_values[idx-1]) / price_values[idx-1];
                    features.push(change);
                } else {
                    features.push(0.0);
                }
            } else {
                features.push(0.0);
            }
        }
        
        // Ensure we have exactly 10 features
        while features.len() < 10 {
            features.push(0.0);
        }
        
        Ok(features)
    }
    
    // LSTM-like prediction using sequence processing
    fn lstm_like_prediction(&mut self, features: &[f64]) -> Result<f64> {
        if features.len() < 3 {
            return Ok(0.0);
        }
        
        // Update hidden state (simplified LSTM cell)
        for (i, &feature) in features.iter().enumerate() {
            if i < self.hidden_state.len() {
                // Forget gate (simplified)
                let forget_factor = 0.8;
                self.hidden_state[i] = self.hidden_state[i] * forget_factor + feature * (1.0 - forget_factor);
            }
        }
        
        // Calculate output using weighted combination of features and hidden state
        let mut output = 0.0;
        
        // Feature contribution
        output += features[0] * self.momentum_weight;      // momentum
        output += (features[1] - 0.5) * self.rsi_weight;  // centered RSI
        output += features[2] * self.volatility_weight;   // volatility
        
        // Hidden state contribution (memory)
        let hidden_contribution: f64 = self.hidden_state.iter().sum::<f64>() / self.hidden_state.len() as f64;
        output += hidden_contribution * 0.2;
        
        // Apply tanh activation for direction (-1 to 1)
        Ok(output.tanh())
    }
    
    // Advanced pattern recognition using multiple pattern filters
    fn advanced_pattern_recognition(&mut self, prices: &[PriceFeed]) -> Result<f64> {
        if prices.len() < 10 {
            return Ok(0.5);
        }
        
        let price_values: Vec<f64> = prices.iter().map(|p| p.price).collect();
        let pattern_features = self.extract_pattern_features(&price_values)?;
        
        // Apply multiple pattern filters
        let mut pattern_scores = Vec::new();
        for (i, &weight) in self.pattern_weights.iter().enumerate() {
            let score = self.apply_pattern_filter(&pattern_features, i)?;
            pattern_scores.push(score * weight);
        }
        
        // Combine pattern scores
        let combined_score = pattern_scores.iter().sum::<f64>() / pattern_scores.len() as f64;
        
        // Normalize to 0-1 range
        Ok((combined_score.tanh() + 1.0) / 2.0)
    }
    
    fn extract_pattern_features(&self, prices: &[f64]) -> Result<Vec<f64>> {
        let mut features = Vec::new();
        
        // Price momentum patterns (last 5 periods)
        for i in 1..prices.len().min(6) {
            let momentum = (prices[prices.len()-i] - prices[prices.len()-i-1]) / prices[prices.len()-i-1];
            features.push(momentum);
        }
        
        // Ensure consistent feature size
        while features.len() < 5 {
            features.push(0.0);
        }
        
        Ok(features)
    }
    
    fn apply_pattern_filter(&self, features: &[f64], filter_index: usize) -> Result<f64> {
        // Simple pattern filter (could be more sophisticated)
        let mut score = 0.0;
        
        match filter_index {
            0 => { // Momentum consistency filter
                let avg_momentum = features.iter().sum::<f64>() / features.len() as f64;
                let consistency = features.iter()
                    .map(|&f| (f - avg_momentum).abs())
                    .sum::<f64>() / features.len() as f64;
                score = 1.0 - consistency.min(1.0);
            }
            1 => { // Trend strength filter
                let trend = features.iter().enumerate()
                    .map(|(i, &f)| f * (i as f64 + 1.0))
                    .sum::<f64>() / features.len() as f64;
                score = trend.abs().min(1.0);
            }
            2 => { // Volatility filter
                let volatility = features.iter()
                    .map(|&f| f.abs())
                    .sum::<f64>() / features.len() as f64;
                score = volatility.min(1.0);
            }
            3 => { // Reversal pattern filter
                let mut reversals = 0;
                for i in 1..features.len() {
                    if features[i] * features[i-1] < 0.0 {
                        reversals += 1;
                    }
                }
                score = (reversals as f64 / features.len() as f64).min(1.0);
            }
            _ => { // Default filter
                score = features.iter().map(|&f| f.abs()).sum::<f64>() / features.len() as f64;
            }
        }
        
        Ok(score)
    }
    
    fn calculate_enhanced_confidence(&self, price_direction: f64, pattern_strength: f64) -> f64 {
        let signal_strength = price_direction.abs();
        let historical_accuracy = if self.total_predictions > 0 {
            self.correct_predictions as f64 / self.total_predictions as f64
        } else {
            0.5
        };
        
        // Weighted combination of factors
        let confidence = (
            signal_strength * 0.4 +
            pattern_strength * 0.3 +
            historical_accuracy * 0.3
        ).min(1.0);
        
        confidence
    }
    
    fn calculate_risk_level(&self, volatility: f64, confidence: f64, pattern_strength: f64) -> f64 {
        // Higher volatility = higher risk
        // Lower confidence = higher risk
        // Lower pattern strength = higher risk
        let volatility_risk = volatility * 10.0;
        let confidence_risk = 1.0 - confidence;
        let pattern_risk = 1.0 - pattern_strength;
        
        let combined_risk = (volatility_risk + confidence_risk + pattern_risk) / 3.0;
        combined_risk.min(1.0)
    }
    
    fn store_pattern_for_learning(&mut self, features: &[f64], confidence: f64) {
        let pattern_match = PatternMatch {
            pattern_features: features.to_vec(),
            confidence,
            outcome: None,
            timestamp: Utc::now(),
        };
        
        self.pattern_memory.push_back(pattern_match);
        if self.pattern_memory.len() > 100 {
            self.pattern_memory.pop_front();
        }
    }
}

// Main neural enhancement system
#[derive(Debug)]
pub struct NeuralEnhancement {
    config: NeuralConfig,
    learner: OnlineLearner,
    enabled: bool,
}

impl NeuralEnhancement {
    pub fn new() -> Result<Self> {
        let config = NeuralConfig::default();
        let learner = OnlineLearner::new(&config);
        
        let enabled = std::env::var("NEURAL_ENABLED")
            .unwrap_or_else(|_| "true".to_string())
            .parse::<bool>()
            .unwrap_or(true);
        
        info!("🧠 Neural Enhancement initialized: {}", if enabled { "ENABLED" } else { "DISABLED" });
        
        Ok(Self {
            config,
            learner,
            enabled,
        })
    }
    
    pub async fn enhance_signal(
        &mut self, 
        signal: &TradingSignal, 
        prices: &[PriceFeed], 
        indicators: &TradingIndicators
    ) -> Result<TradingSignal> {
        if !self.enabled {
            return Ok(signal.clone());
        }
        
        // Get neural prediction
        let neural_pred = self.learner.predict(prices, indicators)?;
        
        // Only enhance if neural network is confident
        if neural_pred.confidence < self.config.confidence_threshold {
            debug!("🧠 Neural confidence too low ({:.1}%), using original signal", 
                   neural_pred.confidence * 100.0);
            return Ok(signal.clone());
        }
        
        let mut enhanced_signal = signal.clone();
        
        // Enhance confidence based on neural prediction alignment
        let signal_direction = match signal.signal_type {
            SignalType::Buy => 1.0,
            SignalType::Sell => -1.0,
            SignalType::Hold => 0.0,
        };
        
        // Check if neural prediction agrees with signal
        let agreement = if signal_direction * neural_pred.price_direction > 0.0 {
            neural_pred.confidence
        } else {
            1.0 - neural_pred.confidence // Disagreement reduces confidence
        };
        
        // Combine original and neural confidence
        let neural_weight = if self.learner.get_accuracy() > 0.6 { 0.4 } else { 0.2 };
        enhanced_signal.confidence = 
            signal.confidence * (1.0 - neural_weight) + 
            agreement * neural_weight;
        
        // Add neural reasoning
        enhanced_signal.reasoning.push(format!(
            "Neural: Dir {:.2}, Conf {:.1}%, Pattern {:.1}%, Risk {:.1}%",
            neural_pred.price_direction,
            neural_pred.confidence * 100.0,
            neural_pred.pattern_strength * 100.0,
            neural_pred.risk_level * 100.0
        ));
        
        // Potentially override signal if neural network is very confident and disagrees
        if neural_pred.confidence > 0.8 && self.learner.get_accuracy() > 0.7 {
            if neural_pred.price_direction > 0.3 && signal.signal_type == SignalType::Sell {
                enhanced_signal.signal_type = SignalType::Buy;
                enhanced_signal.reasoning.push("Neural override: Strong bullish signal".to_string());
                info!("🧠 Neural OVERRIDE: Changed SELL to BUY (confidence: {:.1}%)", 
                      neural_pred.confidence * 100.0);
            } else if neural_pred.price_direction < -0.3 && signal.signal_type == SignalType::Buy {
                enhanced_signal.signal_type = SignalType::Sell;
                enhanced_signal.reasoning.push("Neural override: Strong bearish signal".to_string());
                info!("🧠 Neural OVERRIDE: Changed BUY to SELL (confidence: {:.1}%)", 
                      neural_pred.confidence * 100.0);
            }
        }
        
        info!("🧠 Neural enhancement applied: Original {:.1}% → Enhanced {:.1}%", 
              signal.confidence * 100.0, enhanced_signal.confidence * 100.0);
        
        Ok(enhanced_signal)
    }
    
    pub async fn learn_from_trade(&mut self, outcome: &TradeOutcome) -> Result<()> {
        if !self.enabled {
            return Ok(());
        }
        
        self.learner.learn_from_outcome(outcome)?;
        Ok(())
    }
    
    pub fn get_performance_stats(&self) -> (u64, f64, bool) {
        let (total, accuracy, _, _, _) = self.learner.get_learning_stats();
        (total, accuracy, self.enabled)
    }
    
    pub fn is_ready(&self) -> bool {
        self.enabled && self.learner.total_predictions >= 5
    }
}