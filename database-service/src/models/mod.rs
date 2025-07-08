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
    pub take_profit: Option<f64>,
    pub stop_loss: Option<f64>,
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
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Trade {
    pub id: String,
    pub position_id: String,
    pub trade_type: String,
    pub price: f64,
    pub quantity: f64,
    pub timestamp: DateTime<Utc>,
    pub transaction_hash: Option<String>,
    pub fees: Option<f64>,
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
    pub take_profit: Option<f64>,
    pub stop_loss: Option<f64>,
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

#[derive(Debug, Serialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    pub data: Option<T>,
    pub error: Option<String>,
}

impl<T> ApiResponse<T> {
    pub fn success(data: T) -> Self {
        Self {
            success: true,
            data: Some(data),
            error: None,
        }
    }

    pub fn error(message: String) -> Self {
        Self {
            success: false,
            data: None,
            error: Some(message),
        }
    }
} 