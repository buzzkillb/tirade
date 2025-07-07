use crate::db::Database;
use crate::models::{ApiResponse, CreateWalletRequest, StoreBalanceRequest, StorePriceRequest};
use axum::{
    extract::State,
    http::StatusCode,
    response::Json,
};
use std::sync::Arc;
use tracing::{info, warn};

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