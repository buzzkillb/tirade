use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    pub data: Option<T>,
    pub error: Option<String>,
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
pub struct Candle {
    pub id: String,
    pub pair: String,
    pub interval: String,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: f64,
    pub timestamp: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
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

// New models for enhanced database schema
#[derive(Debug, Clone, Serialize)]
pub struct TechnicalIndicator {
    pub pair: String,
    pub timestamp: DateTime<Utc>,
    pub indicator_type: String,
    pub value: f64,
    pub period: Option<i32>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TradingSignalDb {
    pub pair: String,
    pub timestamp: DateTime<Utc>,
    pub signal_type: String,
    pub confidence: f64,
    pub price: f64,
    pub reasoning: String,
    pub take_profit: f64,
    pub stop_loss: f64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PositionDb {
    pub pair: String,
    pub position_type: String,
    pub entry_price: f64,
    pub entry_time: DateTime<Utc>,
    pub quantity: f64,
    pub status: String,
    pub take_profit: f64,
    pub stop_loss: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct TradeDb {
    pub pair: String,
    pub trade_type: String,
    pub entry_price: f64,
    pub exit_price: f64,
    pub quantity: f64,
    pub entry_time: DateTime<Utc>,
    pub exit_time: DateTime<Utc>,
    pub pnl: f64,
    pub pnl_percent: f64,
    pub signal_id: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TradingConfigDb {
    pub pair: String,
    pub strategy_name: String,
    pub rsi_oversold: f64,
    pub rsi_overbought: f64,
    pub take_profit_threshold: f64,
    pub stop_loss_threshold: f64,
    pub min_confidence: f64,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PerformanceMetricDb {
    pub pair: String,
    pub timestamp: DateTime<Utc>,
    pub total_trades: i32,
    pub winning_trades: i32,
    pub losing_trades: i32,
    pub total_pnl: f64,
    pub win_rate: f64,
    pub avg_win: f64,
    pub avg_loss: f64,
    pub max_drawdown: f64,
    pub sharpe_ratio: f64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct StoreTechnicalIndicatorsRequest {
    pub pair: String,
    pub sma_20: Option<f64>,
    pub sma_50: Option<f64>,
    pub sma_200: Option<f64>,
    pub rsi_14: Option<f64>,
    pub price_change_24h: Option<f64>,
    pub price_change_percent_24h: Option<f64>,
    pub volatility_24h: Option<f64>,
    pub current_price: f64,
} 