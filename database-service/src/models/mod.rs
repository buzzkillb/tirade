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