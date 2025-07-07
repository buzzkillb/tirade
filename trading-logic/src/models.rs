use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    pub data: T,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PriceFeed {
    pub id: String,
    pub source: String,
    pub pair: String,
    pub price: f64,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TechnicalIndicators {
    pub pair: String,
    pub timestamp: DateTime<Utc>,
    pub sma_20: Option<f64>,
    pub sma_50: Option<f64>,
    pub sma_200: Option<f64>,
    pub rsi_14: Option<f64>,
    pub price_change_24h: Option<f64>,
    pub price_change_percent_24h: Option<f64>,
    pub volatility_24h: Option<f64>,
    pub current_price: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct TradingSignal {
    pub signal_type: SignalType,
    pub confidence: f64,
    pub price: f64,
    pub timestamp: DateTime<Utc>,
    pub reasoning: Vec<String>,
    pub take_profit: f64,
    pub stop_loss: f64,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub enum SignalType {
    Buy,
    Sell,
    Hold,
}

#[derive(Debug, Clone, Serialize)]
pub struct TradingIndicators {
    pub rsi_fast: Option<f64>,
    pub rsi_slow: Option<f64>,
    pub sma_short: Option<f64>,
    pub sma_long: Option<f64>,
    pub volatility: Option<f64>,
    pub price_momentum: Option<f64>,
    pub price_change_percent: f64,
}

#[derive(Debug, Clone)]
pub struct Position {
    pub entry_price: f64,
    pub entry_time: DateTime<Utc>,
    pub quantity: f64,
    pub position_type: PositionType,
}

#[derive(Debug, Clone, PartialEq)]
pub enum PositionType {
    Long,
    Short,
} 