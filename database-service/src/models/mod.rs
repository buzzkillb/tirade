use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

// Database models
#[derive(Debug, Serialize, Deserialize)]
pub struct Wallet {
    pub id: String,
    pub address: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BalanceSnapshot {
    pub id: String,
    pub wallet_id: String,
    pub sol_balance: f64,
    pub usdc_balance: f64,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PriceFeed {
    pub id: String,
    pub source: String,
    pub pair: String,
    pub price: f64,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TechnicalIndicators {
    pub id: String,
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
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TradingSignal {
    pub id: String,
    pub pair: String,
    pub signal_type: String,
    pub confidence: f64,
    pub price: f64,
    pub timestamp: DateTime<Utc>,
    pub reasoning: Option<String>,
    pub executed: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Position {
    pub id: String,
    pub wallet_id: String,
    pub pair: String,
    pub position_type: String,
    pub entry_price: f64,
    pub entry_time: DateTime<Utc>,
    pub quantity: f64,
    pub status: String,
    pub exit_price: Option<f64>,
    pub exit_time: Option<DateTime<Utc>>,
    pub pnl: Option<f64>,
    pub pnl_percent: Option<f64>,
    pub duration_seconds: Option<i64>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    // Dashboard compatibility fields
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_price: Option<f64>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Trade {
    pub id: String,
    pub pair: String,
    pub trade_type: String,
    pub price: f64,
    pub quantity: f64,
    pub total_value: f64,
    pub timestamp: DateTime<Utc>,
    pub status: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TradingConfig {
    pub id: String,
    pub name: String,
    pub pair: String,
    pub min_data_points: i32,
    pub check_interval_secs: i32,
    pub take_profit_percent: f64,
    pub stop_loss_percent: f64,
    pub max_position_size: f64,
    pub enabled: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PerformanceMetrics {
    pub id: String,
    pub wallet_id: String,
    pub period: String,
    pub total_trades: i32,
    pub winning_trades: i32,
    pub losing_trades: i32,
    pub total_pnl: f64,
    pub total_pnl_percent: f64,
    pub win_rate: f64,
    pub avg_win: f64,
    pub avg_loss: f64,
    pub max_drawdown: f64,
    pub sharpe_ratio: Option<f64>,
    pub timestamp: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Candle {
    pub id: String,
    pub pair: String,
    pub interval: String, // "30s", "1m", "5m", etc.
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: f64,
    pub timestamp: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MLTradeHistory {
    pub id: String,
    pub pair: String,
    pub entry_price: f64,
    pub exit_price: f64,
    pub pnl: f64,
    pub duration_seconds: i64,
    pub entry_time: DateTime<Utc>,
    pub exit_time: DateTime<Utc>,
    pub success: bool,
    pub market_regime: String,
    pub trend_strength: f64,
    pub volatility: f64,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MLTradeStats {
    pub total_trades: usize,
    pub win_rate: f64,
    pub avg_pnl: f64,
    pub avg_win: f64,
    pub avg_loss: f64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    pub data: T,
    pub message: Option<String>,
}

impl<T> ApiResponse<T> {
    pub fn success(data: T) -> Self {
        Self {
            success: true,
            data,
            message: None,
        }
    }

    pub fn error(message: String) -> Self {
        Self {
            success: false,
            data: unsafe { std::mem::zeroed() },
            message: Some(message),
        }
    }
}

// Request/Response models
#[derive(Debug, Deserialize)]
pub struct CreateWalletRequest {
    pub address: String,
}

#[derive(Debug, Deserialize)]
pub struct StoreBalanceRequest {
    pub wallet_address: String,
    pub sol_balance: f64,
    pub usdc_balance: f64,
}

#[derive(Debug, Deserialize)]
pub struct StorePriceRequest {
    pub source: String,
    pub pair: String,
    pub price: f64,
}

#[derive(Debug, Deserialize)]
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

#[derive(Debug, Deserialize)]
pub struct StoreTradingSignalRequest {
    pub pair: String,
    pub signal_type: String,
    pub confidence: f64,
    pub price: f64,
    pub reasoning: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CreatePositionRequest {
    pub wallet_address: String,
    pub pair: String,
    pub position_type: String,
    pub entry_price: f64,
    pub quantity: f64,
}

#[derive(Debug, Deserialize)]
pub struct ClosePositionRequest {
    pub position_id: String,
    pub exit_price: f64,
    pub transaction_hash: Option<String>,
    pub fees: Option<f64>,
}

#[derive(Debug, Deserialize)]
pub struct CreateTradingConfigRequest {
    pub name: String,
    pub pair: String,
    pub min_data_points: Option<i32>,
    pub check_interval_secs: Option<i32>,
    pub take_profit_percent: Option<f64>,
    pub stop_loss_percent: Option<f64>,
    pub max_position_size: Option<f64>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct StoreCandleRequest {
    pub pair: String,
    pub interval: String,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: f64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UpdatePositionStatusRequest {
    pub position_id: String,
    pub status: String,
    pub exit_price: Option<f64>,
    pub pnl: Option<f64>,
    pub pnl_percent: Option<f64>,
} 

#[derive(Debug, Serialize, Deserialize)]
pub struct AdvancedIndicators {
    pub pair: String,
    pub timestamp: DateTime<Utc>,
    pub bollinger_bands: Option<BollingerBands>,
    pub macd: Option<MACD>,
    pub exponential_smoothing: Option<ExponentialSmoothing>,
    pub stochastic: Option<StochasticOscillator>,
    pub rsi_divergence: Option<f64>,
    pub confluence_score: Option<f64>,
    pub market_regime: Option<String>,
    pub trend_strength: Option<f64>,
    pub volatility_regime: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BollingerBands {
    pub upper: f64,
    pub middle: f64,
    pub lower: f64,
    pub bandwidth: f64,
    pub percent_b: f64,
    pub squeeze: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MACD {
    pub macd_line: f64,
    pub signal_line: f64,
    pub histogram: f64,
    pub bullish_crossover: bool,
    pub bearish_crossover: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ExponentialSmoothing {
    pub ema_12: f64,
    pub ema_26: f64,
    pub ema_50: f64,
    pub smoothed_price: f64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct StochasticOscillator {
    pub k_percent: f64,
    pub d_percent: f64,
    pub overbought: bool,
    pub oversold: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MLPrediction {
    pub pair: String,
    pub timestamp: DateTime<Utc>,
    pub entry_probability: f64,
    pub exit_probability: f64,
    pub confidence_score: f64,
    pub market_regime: String,
    pub optimal_position_size: f64,
    pub risk_score: f64,
    pub signal_quality: String,
    pub historical_accuracy: f64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TradingAnalysis {
    pub pair: String,
    pub timestamp: DateTime<Utc>,
    pub current_price: f64,
    pub technical_indicators: TechnicalIndicators,
    pub advanced_indicators: AdvancedIndicators,
    pub ml_prediction: Option<MLPrediction>,
    pub trading_signal: Option<TradingSignal>,
    pub market_summary: MarketSummary,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MarketSummary {
    pub trend_direction: String,
    pub trend_strength: f64,
    pub volatility_level: String,
    pub support_level: Option<f64>,
    pub resistance_level: Option<f64>,
    pub market_regime: String,
    pub risk_level: String,
    pub optimal_strategy: String,
} 