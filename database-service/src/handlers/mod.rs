use crate::db::Database;
use crate::models::{ApiResponse, CreateWalletRequest, StoreBalanceRequest, StorePriceRequest};
use crate::indicators::{calculate_indicators, TechnicalIndicators};
use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::Json,
};
use serde::Deserialize;
use std::sync::Arc;
use tracing::{info, warn};

#[derive(Debug, Deserialize)]
pub struct PriceHistoryQuery {
    pub hours: Option<i64>,
    pub source: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct IndicatorsQuery {
    pub hours: Option<i64>,
    pub source: Option<String>,
}

pub async fn health_check() -> Json<ApiResponse<String>> {
    Json(ApiResponse::success("Database service is healthy".to_string()))
}

pub async fn create_wallet(
    State(db): State<Arc<Database>>,
    Json(payload): Json<CreateWalletRequest>,
) -> std::result::Result<Json<ApiResponse<crate::models::Wallet>>, StatusCode> {
    if payload.address.trim().is_empty() {
        return Err(StatusCode::BAD_REQUEST);
    }

    match db.create_wallet(&payload.address).await {
        Ok(wallet) => {
            info!("Created wallet: {}", wallet.address);
            Ok(Json(ApiResponse::success(wallet)))
        }
        Err(e) => {
            warn!("Failed to create wallet: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

pub async fn store_balance(
    State(db): State<Arc<Database>>,
    Json(payload): Json<StoreBalanceRequest>,
) -> std::result::Result<Json<ApiResponse<crate::models::BalanceSnapshot>>, StatusCode> {
    // First, ensure wallet exists
    let wallet = match db.get_wallet_by_address(&payload.wallet_address).await {
        Ok(Some(wallet)) => wallet,
        Ok(None) => {
            // Create wallet if it doesn't exist
            match db.create_wallet(&payload.wallet_address).await {
                Ok(wallet) => wallet,
                Err(e) => {
                    warn!("Failed to create wallet: {}", e);
                    return Err(StatusCode::INTERNAL_SERVER_ERROR);
                }
            }
        }
        Err(e) => {
            warn!("Failed to get wallet: {}", e);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };

    // Store balance snapshot
    match db.store_balance(&wallet.id, payload.sol_balance, payload.usdc_balance).await {
        Ok(snapshot) => {
            info!("Stored balance for wallet {}: SOL={}, USDC={}", 
                  payload.wallet_address, payload.sol_balance, payload.usdc_balance);
            Ok(Json(ApiResponse::success(snapshot)))
        }
        Err(e) => {
            warn!("Failed to store balance: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

pub async fn store_price(
    State(db): State<Arc<Database>>,
    Json(payload): Json<StorePriceRequest>,
) -> std::result::Result<Json<ApiResponse<crate::models::PriceFeed>>, StatusCode> {
    match db.store_price(&payload.source, &payload.pair, payload.price).await {
        Ok(price_feed) => {
            info!("Stored price: {} {} = ${}", payload.source, payload.pair, payload.price);
            Ok(Json(ApiResponse::success(price_feed)))
        }
        Err(e) => {
            warn!("Failed to store price: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

pub async fn get_wallet_balances(
    State(db): State<Arc<Database>>,
    axum::extract::Path(address): axum::extract::Path<String>,
) -> std::result::Result<Json<ApiResponse<Vec<crate::models::BalanceSnapshot>>>, StatusCode> {
    match db.get_wallet_balances(&address).await {
        Ok(balances) => {
            Ok(Json(ApiResponse::success(balances)))
        }
        Err(e) => {
            warn!("Failed to get wallet balances: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

pub async fn get_prices(
    State(db): State<Arc<Database>>,
    axum::extract::Path(pair): axum::extract::Path<String>,
) -> std::result::Result<Json<ApiResponse<Vec<crate::models::PriceFeed>>>, StatusCode> {
    match db.get_prices(&pair).await {
        Ok(prices) => {
            Ok(Json(ApiResponse::success(prices)))
        }
        Err(e) => {
            warn!("Failed to get prices: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

pub async fn get_price_history(
    State(db): State<Arc<Database>>,
    axum::extract::Path(pair): axum::extract::Path<String>,
    Query(query): Query<PriceHistoryQuery>,
) -> std::result::Result<Json<ApiResponse<Vec<crate::models::PriceFeed>>>, StatusCode> {
    let hours = query.hours.unwrap_or(24);
    
    match db.get_price_history(&pair, hours).await {
        Ok(prices) => {
            info!("Retrieved {} price records for {} (last {} hours)", prices.len(), pair, hours);
            Ok(Json(ApiResponse::success(prices)))
        }
        Err(e) => {
            warn!("Failed to get price history: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

pub async fn get_latest_price(
    State(db): State<Arc<Database>>,
    axum::extract::Path(pair): axum::extract::Path<String>,
    Query(query): Query<PriceHistoryQuery>,
) -> std::result::Result<Json<ApiResponse<Option<crate::models::PriceFeed>>>, StatusCode> {
    let source = query.source.as_deref();
    
    match db.get_latest_price(&pair, source).await {
        Ok(price) => {
            Ok(Json(ApiResponse::success(price)))
        }
        Err(e) => {
            warn!("Failed to get latest price: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

pub async fn get_technical_indicators(
    State(db): State<Arc<Database>>,
    axum::extract::Path(pair): axum::extract::Path<String>,
    Query(query): Query<IndicatorsQuery>,
) -> std::result::Result<Json<ApiResponse<TechnicalIndicators>>, StatusCode> {
    let hours = query.hours.unwrap_or(24);
    
    match db.get_price_history(&pair, hours).await {
        Ok(prices) => {
            if prices.is_empty() {
                return Err(StatusCode::NOT_FOUND);
            }
            
            let indicators = calculate_indicators(&prices);
            info!("Calculated technical indicators for {} ({} data points)", pair, prices.len());
            Ok(Json(ApiResponse::success(indicators)))
        }
        Err(e) => {
            warn!("Failed to get technical indicators: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
} 