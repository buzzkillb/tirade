use crate::db::Database;
use crate::models::{
    ApiResponse, CreateWalletRequest, StoreBalanceRequest, StorePriceRequest,
    StoreTechnicalIndicatorsRequest, StoreTradingSignalRequest, CreatePositionRequest,
    ClosePositionRequest, CreateTradingConfigRequest
};
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

#[derive(Debug, Deserialize)]
pub struct PositionHistoryQuery {
    pub limit: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct TradingSignalsQuery {
    pub limit: Option<i64>,
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

pub async fn store_technical_indicators(
    State(db): State<Arc<Database>>,
    axum::extract::Path(pair): axum::extract::Path<String>,
    Json(payload): Json<StoreTechnicalIndicatorsRequest>,
) -> std::result::Result<Json<ApiResponse<crate::models::TechnicalIndicators>>, StatusCode> {
    match db.store_technical_indicators(&pair, &payload).await {
        Ok(indicators) => {
            info!("Stored technical indicators for {}: RSI={:?}, SMA20={:?}", 
                  pair, indicators.rsi_14, indicators.sma_20);
            Ok(Json(ApiResponse::success(indicators)))
        }
        Err(e) => {
            warn!("Failed to store technical indicators: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

pub async fn get_latest_technical_indicators(
    State(db): State<Arc<Database>>,
    axum::extract::Path(pair): axum::extract::Path<String>,
) -> std::result::Result<Json<ApiResponse<Option<crate::models::TechnicalIndicators>>>, StatusCode> {
    match db.get_latest_technical_indicators(&pair).await {
        Ok(indicators) => {
            Ok(Json(ApiResponse::success(indicators)))
        }
        Err(e) => {
            warn!("Failed to get latest technical indicators: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

pub async fn store_trading_signal(
    State(db): State<Arc<Database>>,
    Json(payload): Json<StoreTradingSignalRequest>,
) -> std::result::Result<Json<ApiResponse<crate::models::TradingSignal>>, StatusCode> {
    match db.store_trading_signal(&payload).await {
        Ok(signal) => {
            info!("Stored trading signal: {} {} (confidence: {:.1}%)", 
                  signal.signal_type, signal.pair, signal.confidence * 100.0);
            Ok(Json(ApiResponse::success(signal)))
        }
        Err(e) => {
            warn!("Failed to store trading signal: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

pub async fn get_trading_signals(
    State(db): State<Arc<Database>>,
    axum::extract::Path(pair): axum::extract::Path<String>,
    Query(query): Query<TradingSignalsQuery>,
) -> std::result::Result<Json<ApiResponse<Vec<crate::models::TradingSignal>>>, StatusCode> {
    let limit = query.limit.unwrap_or(100);
    
    match db.get_trading_signals(&pair, limit).await {
        Ok(signals) => {
            info!("Retrieved {} trading signals for {}", signals.len(), pair);
            Ok(Json(ApiResponse::success(signals)))
        }
        Err(e) => {
            warn!("Failed to get trading signals: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

pub async fn create_position(
    State(db): State<Arc<Database>>,
    Json(payload): Json<CreatePositionRequest>,
) -> std::result::Result<Json<ApiResponse<crate::models::Position>>, StatusCode> {
    match db.create_position(&payload).await {
        Ok(position) => {
            info!("Created position: {} {} at ${:.4} (quantity: {})", 
                  position.position_type, position.pair, position.entry_price, position.quantity);
            Ok(Json(ApiResponse::success(position)))
        }
        Err(e) => {
            warn!("Failed to create position: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

pub async fn close_position(
    State(db): State<Arc<Database>>,
    Json(payload): Json<ClosePositionRequest>,
) -> std::result::Result<Json<ApiResponse<crate::models::Position>>, StatusCode> {
    match db.close_position(&payload).await {
        Ok(position) => {
            if let (Some(pnl), Some(pnl_percent)) = (position.pnl, position.pnl_percent) {
                let emoji = if pnl > 0.0 { "💰" } else if pnl < 0.0 { "💸" } else { "➡️" };
                info!("Closed position: {} {} at ${:.4} (PnL: {} ${:.2}, {:.2}%)", 
                      position.position_type, position.pair, position.exit_price.unwrap_or(0.0),
                      emoji, pnl, pnl_percent);
            }
            Ok(Json(ApiResponse::success(position)))
        }
        Err(e) => {
            warn!("Failed to close position: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

pub async fn get_open_positions(
    State(db): State<Arc<Database>>,
    axum::extract::Path(address): axum::extract::Path<String>,
) -> std::result::Result<Json<ApiResponse<Vec<crate::models::Position>>>, StatusCode> {
    match db.get_open_positions(&address).await {
        Ok(positions) => {
            info!("Retrieved {} open positions for wallet {}", positions.len(), address);
            Ok(Json(ApiResponse::success(positions)))
        }
        Err(e) => {
            warn!("Failed to get open positions: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

pub async fn get_position_history(
    State(db): State<Arc<Database>>,
    axum::extract::Path(address): axum::extract::Path<String>,
    Query(query): Query<PositionHistoryQuery>,
) -> std::result::Result<Json<ApiResponse<Vec<crate::models::Position>>>, StatusCode> {
    let limit = query.limit.unwrap_or(100);
    
    match db.get_position_history(&address, limit).await {
        Ok(positions) => {
            info!("Retrieved {} position history records for wallet {}", positions.len(), address);
            Ok(Json(ApiResponse::success(positions)))
        }
        Err(e) => {
            warn!("Failed to get position history: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

pub async fn create_trading_config(
    State(db): State<Arc<Database>>,
    Json(payload): Json<CreateTradingConfigRequest>,
) -> std::result::Result<Json<ApiResponse<crate::models::TradingConfig>>, StatusCode> {
    match db.create_trading_config(&payload).await {
        Ok(config) => {
            info!("Created trading config: {} for {} (TP: {:.1}%, SL: {:.1}%)", 
                  config.name, config.pair, config.take_profit_percent, config.stop_loss_percent);
            Ok(Json(ApiResponse::success(config)))
        }
        Err(e) => {
            warn!("Failed to create trading config: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

pub async fn get_trading_config(
    State(db): State<Arc<Database>>,
    axum::extract::Path(name): axum::extract::Path<String>,
) -> std::result::Result<Json<ApiResponse<Option<crate::models::TradingConfig>>>, StatusCode> {
    match db.get_trading_config(&name).await {
        Ok(config) => {
            Ok(Json(ApiResponse::success(config)))
        }
        Err(e) => {
            warn!("Failed to get trading config: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

pub async fn get_open_positions_by_pair(
    State(db): State<Arc<Database>>,
    axum::extract::Path(pair): axum::extract::Path<String>,
) -> std::result::Result<Json<ApiResponse<Option<crate::models::Position>>>, StatusCode> {
    match db.get_open_positions_by_pair(&pair).await {
        Ok(position) => {
            Ok(Json(ApiResponse::success(position)))
        }
        Err(e) => {
            warn!("Failed to get open positions by pair: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

pub async fn update_position_status(
    State(db): State<Arc<Database>>,
    axum::extract::Path(position_id): axum::extract::Path<String>,
    Json(payload): Json<serde_json::Value>,
) -> std::result::Result<Json<ApiResponse<crate::models::Position>>, StatusCode> {
    let status = payload["status"].as_str()
        .ok_or(StatusCode::BAD_REQUEST)?;
    
    match db.update_position_status(&position_id, status).await {
        Ok(position) => {
            info!("Updated position {} status to {}", position_id, status);
            Ok(Json(ApiResponse::success(position)))
        }
        Err(e) => {
            warn!("Failed to update position status: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
} 