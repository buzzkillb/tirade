use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use std::sync::Arc;
use tracing::{info, warn, error};
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use crate::db::Database;
use crate::models::{
    ApiResponse, CreateWalletRequest, StoreBalanceRequest, StorePriceRequest,
    StoreTechnicalIndicatorsRequest, StoreTradingSignalRequest, CreatePositionRequest,
    ClosePositionRequest, CreateTradingConfigRequest, StoreCandleRequest,
    UpdatePositionStatusRequest, MLTradeHistory, MLTradeStats,
};
use crate::indicators::{calculate_indicators, TechnicalIndicators};
use serde_json::Value;

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

pub async fn get_recent_trading_signals(
    State(db): State<Arc<Database>>,
    Query(query): Query<TradingSignalsQuery>,
) -> std::result::Result<Json<ApiResponse<Vec<crate::models::TradingSignal>>>, StatusCode> {
    let limit = query.limit.unwrap_or(5);
    
    match db.get_recent_trading_signals(limit).await {
        Ok(signals) => {
            info!("Retrieved {} recent trading signals", signals.len());
            Ok(Json(ApiResponse::success(signals)))
        }
        Err(e) => {
            warn!("Failed to get recent trading signals: {}", e);
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
    info!("🔴 Closing position: ID={}, Exit Price=${:.4}", payload.position_id, payload.exit_price);
    
    match db.close_position(&payload).await {
        Ok(position) => {
            if let (Some(pnl), Some(pnl_percent)) = (position.pnl, position.pnl_percent) {
                let emoji = if pnl > 0.0 { "💰" } else if pnl < 0.0 { "💸" } else { "➡️" };
                info!("✅ Closed position: {} {} at ${:.4} (PnL: {} ${:.2}, {:.2}%)", 
                      position.position_type, position.pair, position.exit_price.unwrap_or(0.0),
                      emoji, pnl, pnl_percent);
            } else {
                info!("✅ Closed position: {} {} at ${:.4}", 
                      position.position_type, position.pair, position.exit_price.unwrap_or(0.0));
            }
            Ok(Json(ApiResponse::success(position)))
        }
        Err(e) => {
            error!("❌ Failed to close position {}: {}", payload.position_id, e);
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

pub async fn get_all_positions(
    State(db): State<Arc<Database>>,
    Query(query): Query<PositionHistoryQuery>,
) -> std::result::Result<Json<ApiResponse<Vec<crate::models::Position>>>, StatusCode> {
    let limit = query.limit.unwrap_or(100);
    
    match db.get_all_positions(limit).await {
        Ok(positions) => {
            info!("Retrieved {} all positions for dashboard", positions.len());
            Ok(Json(ApiResponse::success(positions)))
        }
        Err(e) => {
            warn!("Failed to get all positions: {}", e);
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

// Dashboard-specific endpoints
#[derive(Debug, Deserialize)]
pub struct SignalCountQuery {
    pub hours: Option<i64>,
}

pub async fn get_signals_count(
    State(db): State<Arc<Database>>,
    axum::extract::Path(pair): axum::extract::Path<String>,
    Query(query): Query<SignalCountQuery>,
) -> std::result::Result<Json<ApiResponse<Value>>, StatusCode> {
    let hours = query.hours.unwrap_or(24);
    
    match db.get_signals_count(&pair, hours).await {
        Ok(count) => {
            let response = serde_json::json!({
                "count": count,
                "pair": pair,
                "hours": hours
            });
            Ok(Json(ApiResponse::success(response)))
        }
        Err(e) => {
            warn!("Failed to get signals count: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

pub async fn get_active_positions_dashboard(
    State(db): State<Arc<Database>>,
) -> std::result::Result<Json<ApiResponse<Vec<crate::models::Position>>>, StatusCode> {
    match db.get_all_active_positions().await {
        Ok(positions) => {
            info!("Retrieved {} active positions for dashboard", positions.len());
            Ok(Json(ApiResponse::success(positions)))
        }
        Err(e) => {
            warn!("Failed to get active positions for dashboard: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

pub async fn get_recent_trades(
    State(db): State<Arc<Database>>,
) -> std::result::Result<Json<ApiResponse<Vec<crate::models::Trade>>>, StatusCode> {
    match db.get_recent_trades(10).await {
        Ok(trades) => {
            info!("Retrieved {} recent trades for dashboard", trades.len());
            Ok(Json(ApiResponse::success(trades)))
        }
        Err(e) => {
            warn!("Failed to get recent trades: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

pub async fn get_performance_metrics(
    State(db): State<Arc<Database>>,
) -> std::result::Result<Json<ApiResponse<Value>>, StatusCode> {
    match db.get_performance_metrics().await {
        Ok(metrics) => {
            Ok(Json(ApiResponse::success(metrics)))
        }
        Err(e) => {
            warn!("Failed to get performance metrics: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

pub async fn get_wallet_performance_metrics(
    State(db): State<Arc<Database>>,
) -> std::result::Result<Json<ApiResponse<Value>>, StatusCode> {
    match db.get_wallet_performance_metrics().await {
        Ok(metrics) => {
            info!("Retrieved wallet performance metrics");
            Ok(Json(ApiResponse::success(metrics)))
        }
        Err(e) => {
            warn!("Failed to get wallet performance metrics: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

pub async fn get_candles(
    State(db): State<Arc<Database>>,
    axum::extract::Path((pair, interval)): axum::extract::Path<(String, String)>,
    Query(query): Query<serde_json::Value>,
) -> std::result::Result<Json<ApiResponse<Vec<crate::models::Candle>>>, StatusCode> {
    let limit = query.get("limit")
        .and_then(|v| v.as_i64())
        .unwrap_or(100);
    
    match db.get_candles(&pair, &interval, limit).await {
        Ok(candles) => {
            info!("Retrieved {} candles for {} {} interval", candles.len(), pair, interval);
            Ok(Json(ApiResponse::success(candles)))
        }
        Err(e) => {
            warn!("Failed to get candles: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

pub async fn get_latest_candle(
    State(db): State<Arc<Database>>,
    axum::extract::Path((pair, interval)): axum::extract::Path<(String, String)>,
) -> std::result::Result<Json<ApiResponse<Option<crate::models::Candle>>>, StatusCode> {
    match db.get_latest_candle(&pair, &interval).await {
        Ok(candle) => {
            Ok(Json(ApiResponse::success(candle)))
        }
        Err(e) => {
            warn!("Failed to get latest candle: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

pub async fn store_candle(
    State(db): State<Arc<Database>>,
    Json(payload): Json<serde_json::Value>,
) -> std::result::Result<Json<ApiResponse<crate::models::Candle>>, StatusCode> {
    let pair = payload.get("pair")
        .and_then(|v| v.as_str())
        .ok_or(StatusCode::BAD_REQUEST)?;
    
    let interval = payload.get("interval")
        .and_then(|v| v.as_str())
        .ok_or(StatusCode::BAD_REQUEST)?;
    
    let open = payload.get("open")
        .and_then(|v| v.as_f64())
        .ok_or(StatusCode::BAD_REQUEST)?;
    
    let high = payload.get("high")
        .and_then(|v| v.as_f64())
        .ok_or(StatusCode::BAD_REQUEST)?;
    
    let low = payload.get("low")
        .and_then(|v| v.as_f64())
        .ok_or(StatusCode::BAD_REQUEST)?;
    
    let close = payload.get("close")
        .and_then(|v| v.as_f64())
        .ok_or(StatusCode::BAD_REQUEST)?;
    
    let volume = payload.get("volume")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);
    
    match db.store_candle(pair, interval, open, high, low, close, volume).await {
        Ok(candle) => {
            info!("Stored candle: {} {} O={:.4}, H={:.4}, L={:.4}, C={:.4}", 
                  interval, pair, open, high, low, close);
            Ok(Json(ApiResponse::success(candle)))
        }
        Err(e) => {
            warn!("Failed to store candle: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
} 

pub async fn get_ml_status(
    State(db): State<Arc<Database>>,
) -> std::result::Result<Json<ApiResponse<serde_json::Value>>, StatusCode> {
    // For now, return a default ML status since we don't have ML data in the database yet
    // In the future, this could fetch from a dedicated ML status table or from the trading logic service
    
    let ml_status = serde_json::json!({
        "enabled": std::env::var("ML_ENABLED").unwrap_or_else(|_| "true".to_string()).parse::<bool>().unwrap_or(true),
        "min_confidence": std::env::var("ML_MIN_CONFIDENCE").unwrap_or_else(|_| "0.75".to_string()).parse::<f64>().unwrap_or(0.75),
        "max_position_size": std::env::var("ML_MAX_POSITION_SIZE").unwrap_or_else(|_| "0.9".to_string()).parse::<f64>().unwrap_or(0.9),
        "total_trades": 0, // Will be updated when ML trades are tracked
        "win_rate": 0.0,   // Will be updated when ML trades are tracked
        "avg_pnl": 0.0,    // Will be updated when ML trades are tracked
    });
    
    Ok(Json(ApiResponse::success(ml_status)))
}

pub async fn store_ml_trade_history(
    State(db): State<Arc<Database>>,
    Json(payload): Json<MLTradeHistory>,
) -> std::result::Result<Json<ApiResponse<MLTradeHistory>>, StatusCode> {
    match db.store_ml_trade_history(&payload).await {
        Ok(_) => {
            info!("Stored ML trade history: {} {} -> {} (PnL: {:.2}%)", 
                  payload.pair, payload.entry_price, payload.exit_price, payload.pnl * 100.0);
            Ok(Json(ApiResponse::success(payload)))
        }
        Err(e) => {
            warn!("Failed to store ML trade history: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

pub async fn get_ml_trade_history(
    State(db): State<Arc<Database>>,
    Path(pair): Path<String>,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> std::result::Result<Json<ApiResponse<Vec<MLTradeHistory>>>, StatusCode> {
    let limit = params.get("limit")
        .and_then(|s| s.parse::<i32>().ok())
        .unwrap_or(50);

    match db.get_ml_trade_history(&pair, Some(limit)).await {
        Ok(trades) => {
            info!("Retrieved {} ML trade history records for {}", trades.len(), pair);
            Ok(Json(ApiResponse::success(trades)))
        }
        Err(e) => {
            warn!("Failed to get ML trade history: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

pub async fn get_ml_trade_stats(
    State(db): State<Arc<Database>>,
    Path(pair): Path<String>,
) -> std::result::Result<Json<ApiResponse<MLTradeStats>>, StatusCode> {
    match db.get_ml_trade_stats(&pair).await {
        Ok(stats) => {
            info!("Retrieved ML trade stats for {}: {} trades, {:.1}% win rate", 
                  pair, stats.total_trades, stats.win_rate * 100.0);
            Ok(Json(ApiResponse::success(stats)))
        }
        Err(e) => {
            warn!("Failed to get ML trade stats: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
} 

// New handlers for advanced indicators and trading analysis

pub async fn get_advanced_indicators(
    State(db): State<Arc<Database>>,
    Path(pair): Path<String>,
    Query(query): Query<IndicatorsQuery>,
) -> std::result::Result<Json<ApiResponse<crate::models::AdvancedIndicators>>, StatusCode> {
    let hours = query.hours.unwrap_or(24);
    
    match db.get_price_history(&pair, hours).await {
        Ok(prices) => {
            if prices.is_empty() {
                return Err(StatusCode::NOT_FOUND);
            }
            
            // Calculate advanced indicators from price data
            let advanced_indicators = calculate_advanced_indicators(&prices);
            info!("Calculated advanced indicators for {} ({} data points)", pair, prices.len());
            Ok(Json(ApiResponse::success(advanced_indicators)))
        }
        Err(e) => {
            warn!("Failed to get advanced indicators: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

pub async fn get_ml_predictions(
    State(db): State<Arc<Database>>,
    Path(pair): Path<String>,
) -> std::result::Result<Json<ApiResponse<Option<crate::models::MLPrediction>>>, StatusCode> {
    // Get recent price data and calculate ML predictions
    match db.get_price_history(&pair, 24).await {
        Ok(prices) => {
            if prices.is_empty() {
                return Ok(Json(ApiResponse::success(None)));
            }
            
            let ml_prediction = calculate_ml_prediction(&prices);
            info!("Calculated ML predictions for {}", pair);
            Ok(Json(ApiResponse::success(ml_prediction)))
        }
        Err(e) => {
            warn!("Failed to get ML predictions: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

pub async fn get_trading_analysis(
    State(db): State<Arc<Database>>,
    Path(pair): Path<String>,
) -> std::result::Result<Json<ApiResponse<crate::models::TradingAnalysis>>, StatusCode> {
    // Get comprehensive trading analysis including all indicators and predictions
    match db.get_price_history(&pair, 24).await {
        Ok(prices) => {
            if prices.is_empty() {
                return Err(StatusCode::NOT_FOUND);
            }
            
            let trading_analysis = calculate_comprehensive_analysis(&prices);
            info!("Generated comprehensive trading analysis for {}", pair);
            Ok(Json(ApiResponse::success(trading_analysis)))
        }
        Err(e) => {
            warn!("Failed to get trading analysis: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

pub async fn get_market_summary(
    State(db): State<Arc<Database>>,
    Path(pair): Path<String>,
) -> std::result::Result<Json<ApiResponse<crate::models::MarketSummary>>, StatusCode> {
    match db.get_price_history(&pair, 24).await {
        Ok(prices) => {
            if prices.is_empty() {
                return Err(StatusCode::NOT_FOUND);
            }
            
            let market_summary = calculate_market_summary(&prices);
            info!("Generated market summary for {}", pair);
            Ok(Json(ApiResponse::success(market_summary)))
        }
        Err(e) => {
            warn!("Failed to get market summary: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

// Helper functions for calculating advanced indicators

fn calculate_advanced_indicators(prices: &[crate::models::PriceFeed]) -> crate::models::AdvancedIndicators {
    let price_values: Vec<f64> = prices.iter().map(|p| p.price).collect();
    let current_price = price_values.last().unwrap_or(&0.0);
    
    // Calculate Bollinger Bands
    let bollinger_bands = calculate_bollinger_bands(&price_values, 20, 2.0);
    
    // Calculate MACD
    let macd = calculate_macd(&price_values, 12, 26, 9);
    
    // Calculate Exponential Smoothing
    let exponential_smoothing = calculate_exponential_smoothing(&price_values);
    
    // Calculate Stochastic Oscillator
    let stochastic = calculate_stochastic(&price_values, 14, 3);
    
    // Calculate RSI Divergence (simplified)
    let rsi_divergence = calculate_rsi_divergence(&price_values);
    
    // Calculate Confluence Score
    let confluence_score = calculate_confluence_score(&price_values);
    
    // Determine market regime
    let market_regime = determine_market_regime(&price_values).unwrap_or_else(|| "Unknown".to_string());
    let trend_strength = calculate_trend_strength(&price_values).unwrap_or(0.0);
    let volatility_regime = determine_volatility_regime(&price_values);
    
    crate::models::AdvancedIndicators {
        pair: prices.first().map(|p| p.pair.clone()).unwrap_or_default(),
        timestamp: chrono::Utc::now(),
        bollinger_bands,
        macd,
        exponential_smoothing,
        stochastic,
        rsi_divergence,
        confluence_score,
        market_regime: Some(market_regime),
        trend_strength: Some(trend_strength),
        volatility_regime,
    }
}

fn calculate_bollinger_bands(prices: &[f64], period: usize, std_dev: f64) -> Option<crate::models::BollingerBands> {
    if prices.len() < period {
        return None;
    }
    
    let recent_prices = &prices[prices.len() - period..];
    let sma = recent_prices.iter().sum::<f64>() / period as f64;
    
    let variance = recent_prices.iter()
        .map(|&price| (price - sma).powi(2))
        .sum::<f64>() / period as f64;
    let std = variance.sqrt();
    
    let upper = sma + (std_dev * std);
    let lower = sma - (std_dev * std);
    let bandwidth = (upper - lower) / sma;
    let percent_b = (prices.last().unwrap() - lower) / (upper - lower);
    let squeeze = bandwidth < 0.05;
    
    Some(crate::models::BollingerBands {
        upper,
        middle: sma,
        lower,
        bandwidth,
        percent_b,
        squeeze,
    })
}

fn calculate_macd(prices: &[f64], fast_period: usize, slow_period: usize, signal_period: usize) -> Option<crate::models::MACD> {
    if prices.len() < slow_period {
        return None;
    }
    
    let ema_fast = calculate_ema(prices, fast_period)?;
    let ema_slow = calculate_ema(prices, slow_period)?;
    let macd_line = ema_fast - ema_slow;
    
    // Simplified signal line calculation
    let signal_line = macd_line * 0.8; // Simplified
    let histogram = macd_line - signal_line;
    
    let bullish_crossover = macd_line > signal_line && histogram > 0.0;
    let bearish_crossover = macd_line < signal_line && histogram < 0.0;
    
    Some(crate::models::MACD {
        macd_line,
        signal_line,
        histogram,
        bullish_crossover,
        bearish_crossover,
    })
}

fn calculate_exponential_smoothing(prices: &[f64]) -> Option<crate::models::ExponentialSmoothing> {
    if prices.len() < 50 {
        return None;
    }
    
    let ema_12 = calculate_ema(prices, 12)?;
    let ema_26 = calculate_ema(prices, 26)?;
    let ema_50 = calculate_ema(prices, 50)?;
    
    Some(crate::models::ExponentialSmoothing {
        ema_12,
        ema_26,
        ema_50,
        smoothed_price: ema_12,
    })
}

fn calculate_stochastic(prices: &[f64], k_period: usize, d_period: usize) -> Option<crate::models::StochasticOscillator> {
    if prices.len() < k_period {
        return None;
    }
    
    let recent_prices = &prices[prices.len() - k_period..];
    let highest_high = recent_prices.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b));
    let lowest_low = recent_prices.iter().fold(f64::INFINITY, |a, &b| a.min(b));
    let current_price = prices.last().unwrap();
    
    let k_percent = if highest_high == lowest_low {
        50.0
    } else {
        ((current_price - lowest_low) / (highest_high - lowest_low)) * 100.0
    };
    
    let d_percent = k_percent; // Simplified - would need more data for proper %D calculation
    
    let overbought = k_percent > 80.0;
    let oversold = k_percent < 20.0;
    
    Some(crate::models::StochasticOscillator {
        k_percent,
        d_percent,
        overbought,
        oversold,
    })
}

fn calculate_ema(prices: &[f64], period: usize) -> Option<f64> {
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

fn calculate_rsi_divergence(prices: &[f64]) -> Option<f64> {
    if prices.len() < 14 {
        return None;
    }
    
    // Simplified RSI divergence calculation
    let rsi = calculate_rsi(prices, 14)?;
    let price_momentum = if prices.len() >= 2 {
        (prices.last().unwrap() - prices[prices.len() - 2]) / prices[prices.len() - 2]
    } else {
        0.0
    };
    
    // Simple divergence detection
    if rsi > 70.0 && price_momentum < 0.0 {
        Some(-1.0) // Bearish divergence
    } else if rsi < 30.0 && price_momentum > 0.0 {
        Some(1.0) // Bullish divergence
    } else {
        Some(0.0) // No divergence
    }
}

fn calculate_confluence_score(prices: &[f64]) -> Option<f64> {
    if prices.len() < 20 {
        return None;
    }
    let mut score: f64 = 0.0;
    // RSI confluence
    if let Some(rsi) = calculate_rsi(prices, 14) {
        if rsi < 30.0 || rsi > 70.0 {
            score += 0.3;
        }
    }
    // Trend confluence
    let sma_short = calculate_sma(prices, 20);
    let sma_long = calculate_sma(prices, 50);
    if let (Some(short), Some(long)) = (sma_short, sma_long) {
        let last_price = *prices.last().unwrap();
        if (short > long && last_price > short) || (short < long && last_price < short) {
            score += 0.3;
        }
    }
    // Volatility confluence
    let volatility = calculate_volatility(prices, 20);
    if let Some(vol) = volatility {
        if vol > 0.02 && vol < 0.08 {
            score += 0.2;
        }
    }
    // Momentum confluence
    let momentum = if prices.len() >= 2 {
        (prices.last().unwrap() - prices[prices.len() - 2]) / prices[prices.len() - 2]
    } else {
        0.0
    };
    if momentum.abs() > 0.01 {
        score += 0.2;
    }
    Some(score.min(1.0_f64))
}

fn calculate_rsi(prices: &[f64], period: usize) -> Option<f64> {
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

fn calculate_sma(prices: &[f64], period: usize) -> Option<f64> {
    if prices.len() < period {
        return None;
    }
    
    let sum: f64 = prices.iter().rev().take(period).sum();
    Some(sum / period as f64)
}

fn calculate_volatility(prices: &[f64], window: usize) -> Option<f64> {
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

fn determine_market_regime(prices: &[f64]) -> Option<String> {
    if prices.len() < 20 {
        return None;
    }
    
    let volatility = calculate_volatility(prices, 20)?;
    let trend_strength = calculate_trend_strength(prices).unwrap_or(0.0);
    
    if volatility > 0.05 {
        Some("Volatile".to_string())
    } else if trend_strength > 0.7 {
        Some("Trending".to_string())
    } else if trend_strength < 0.3 {
        Some("Consolidating".to_string())
    } else {
        Some("Ranging".to_string())
    }
}

fn calculate_trend_strength(prices: &[f64]) -> Option<f64> {
    if prices.len() < 20 {
        return None;
    }
    
    let sma_short = calculate_sma(prices, 20)?;
    let sma_long = calculate_sma(prices, 50)?;
    let current_price = prices.last().unwrap();
    
    let price_vs_short = (current_price - sma_short) / sma_short;
    let short_vs_long = (sma_short - sma_long) / sma_long;
    
    let trend_strength = (price_vs_short.abs() + short_vs_long.abs()) / 2.0;
    Some(trend_strength.min(1.0))
}

fn determine_volatility_regime(prices: &[f64]) -> Option<String> {
    if prices.len() < 20 {
        return None;
    }
    
    let volatility = calculate_volatility(prices, 20)?;
    
    if volatility > 0.08 {
        Some("High".to_string())
    } else if volatility > 0.03 {
        Some("Medium".to_string())
    } else {
        Some("Low".to_string())
    }
}

fn calculate_ml_prediction(prices: &[crate::models::PriceFeed]) -> Option<crate::models::MLPrediction> {
    if prices.is_empty() {
        return None;
    }
    
    let current_price = prices.last().unwrap().price;
    let price_values: Vec<f64> = prices.iter().map(|p| p.price).collect();
    
    // Simplified ML prediction calculation
    let entry_probability = calculate_entry_probability(&price_values);
    let exit_probability = calculate_exit_probability(&price_values);
    let confidence_score = calculate_confidence_score(&price_values).unwrap_or(0.5);
    let market_regime = determine_market_regime(&price_values).unwrap_or_else(|| "Unknown".to_string());
    let optimal_position_size = calculate_optimal_position_size(&price_values);
    let risk_score = calculate_risk_score(&price_values);
    let signal_quality = determine_signal_quality(&price_values);
    let historical_accuracy = calculate_historical_accuracy(&price_values);
    
    Some(crate::models::MLPrediction {
        pair: prices.first().unwrap().pair.clone(),
        timestamp: chrono::Utc::now(),
        entry_probability,
        exit_probability,
        confidence_score,
        market_regime,
        optimal_position_size,
        risk_score,
        signal_quality,
        historical_accuracy,
    })
}

fn calculate_entry_probability(prices: &[f64]) -> f64 {
    if prices.len() < 14 {
        return 0.5;
    }
    
    let mut probability: f64 = 0.5;
    
    if let Some(rsi) = calculate_rsi(prices, 14) {
        if rsi < 30.0 { probability += 0.2; }
        if rsi > 70.0 { probability -= 0.2; }
    }
    
    if let Some(bb) = calculate_bollinger_bands(prices, 20, 2.0) {
        if bb.percent_b < 0.2 { probability += 0.15; }
        if bb.percent_b > 0.8 { probability -= 0.15; }
    }
    
    probability.max(0.0_f64).min(1.0_f64)
}

fn calculate_exit_probability(prices: &[f64]) -> f64 {
    if prices.len() < 14 {
        return 0.3;
    }
    
    let mut probability: f64 = 0.3;
    
    if let Some(rsi) = calculate_rsi(prices, 14) {
        if rsi > 70.0 { probability += 0.2; }
        if rsi < 30.0 { probability += 0.2; }
    }
    
    probability.max(0.0_f64).min(1.0_f64)
}

fn calculate_confidence_score(prices: &[f64]) -> Option<f64> {
    if prices.len() < 20 {
        return Some(0.5);
    }
    let mut confidence: f64 = 0.5;
    if let Some(volatility) = calculate_volatility(prices, 20) {
        if volatility > 0.01 && volatility < 0.05 {
            confidence += 0.2;
        }
    }
    if let Some(trend_strength) = calculate_trend_strength(prices) {
        if trend_strength > 0.7 {
            confidence += 0.2;
        }
    }
    Some(confidence.max(0.0).min(1.0))
}

fn calculate_optimal_position_size(prices: &[f64]) -> f64 {
    if prices.len() < 20 {
        return 0.5;
    }
    let mut size: f64 = 0.5;
    let confidence = calculate_confidence_score(prices).unwrap_or(0.5);
    size = confidence;
    if let Some(volatility) = calculate_volatility(prices, 20) {
        if volatility > 0.05 {
            size *= 0.7; // Reduce size in high volatility
        }
    }
    size.max(0.1).min(0.9)
}

fn calculate_risk_score(prices: &[f64]) -> f64 {
    if prices.len() < 20 {
        return 0.5;
    }
    
    let mut risk = 0.5;
    
    if let Some(volatility) = calculate_volatility(prices, 20) {
        risk = volatility * 10.0; // Scale volatility to risk
    }
    
    risk.max(0.0).min(1.0)
}

fn determine_signal_quality(prices: &[f64]) -> String {
    if prices.len() < 20 {
        return "Unknown".to_string();
    }
    
    let confidence = calculate_confidence_score(prices).unwrap_or(0.5);
    
    if confidence > 0.8 {
        "Excellent".to_string()
    } else if confidence > 0.6 {
        "Good".to_string()
    } else if confidence > 0.4 {
        "Fair".to_string()
    } else {
        "Poor".to_string()
    }
}

fn calculate_historical_accuracy(prices: &[f64]) -> f64 {
    // Simplified historical accuracy calculation
    if prices.len() < 20 {
        return 0.5;
    }
    
    let mut accuracy: f64 = 0.5;
    
    if let Some(confidence) = calculate_confidence_score(prices) {
        accuracy = confidence * 0.8 + 0.2; // Base accuracy with confidence boost
    }
    
    accuracy.max(0.0).min(1.0)
}

fn calculate_comprehensive_analysis(prices: &[crate::models::PriceFeed]) -> crate::models::TradingAnalysis {
    let current_price = prices.last().unwrap().price;
    let technical_indicators_db = calculate_indicators(prices);
    let technical_indicators = crate::models::TechnicalIndicators {
        id: "dummy-id".to_string(),
        pair: technical_indicators_db.pair.clone(),
        timestamp: technical_indicators_db.timestamp,
        sma_20: technical_indicators_db.sma_20,
        sma_50: technical_indicators_db.sma_50,
        sma_200: technical_indicators_db.sma_200,
        rsi_14: technical_indicators_db.rsi_14,
        price_change_24h: technical_indicators_db.price_change_24h,
        price_change_percent_24h: technical_indicators_db.price_change_percent_24h,
        volatility_24h: technical_indicators_db.volatility_24h,
        current_price: technical_indicators_db.current_price,
        created_at: chrono::Utc::now(),
    };
    let advanced_indicators = calculate_advanced_indicators(prices);
    let ml_prediction = calculate_ml_prediction(prices);
    let trading_signal = None;
    let market_summary = calculate_market_summary(prices);
    crate::models::TradingAnalysis {
        pair: prices.first().unwrap().pair.clone(),
        timestamp: chrono::Utc::now(),
        current_price,
        technical_indicators,
        advanced_indicators,
        ml_prediction,
        trading_signal,
        market_summary,
    }
}

fn calculate_market_summary(prices: &[crate::models::PriceFeed]) -> crate::models::MarketSummary {
    let price_values: Vec<f64> = prices.iter().map(|p| p.price).collect();
    let current_price = price_values.last().unwrap();
    let trend_direction = if let (Some(sma_short), Some(sma_long)) = (calculate_sma(&price_values, 20), calculate_sma(&price_values, 50)) {
        if *current_price > sma_short && sma_short > sma_long {
            "Uptrend".to_string()
        } else if *current_price < sma_short && sma_short < sma_long {
            "Downtrend".to_string()
        } else {
            "Sideways".to_string()
        }
    } else {
        "Unknown".to_string()
    };
    let trend_strength = calculate_trend_strength(&price_values).unwrap_or(0.5);
    let volatility_level = if let Some(volatility) = calculate_volatility(&price_values, 20) {
        if volatility > 0.05 {
            "High".to_string()
        } else if volatility > 0.02 {
            "Medium".to_string()
        } else {
            "Low".to_string()
        }
    } else {
        "Unknown".to_string()
    };
    let support_level = if let Some(bb) = calculate_bollinger_bands(&price_values, 20, 2.0) {
        Some(bb.lower)
    } else {
        None
    };
    let resistance_level = if let Some(bb) = calculate_bollinger_bands(&price_values, 20, 2.0) {
        Some(bb.upper)
    } else {
        None
    };
    let market_regime = determine_market_regime(&price_values).unwrap_or_else(|| "Unknown".to_string());
    let risk_level = if let Some(volatility) = calculate_volatility(&price_values, 20) {
        if volatility > 0.05 {
            "High".to_string()
        } else if volatility > 0.02 {
            "Medium".to_string()
        } else {
            "Low".to_string()
        }
    } else {
        "Unknown".to_string()
    };
    let optimal_strategy = if trend_strength > 0.7 {
        "Trend Following".to_string()
    } else if let Some(volatility) = calculate_volatility(&price_values, 20) {
        if volatility > 0.05 {
            "Mean Reversion".to_string()
        } else {
            "Range Trading".to_string()
        }
    } else {
        "Conservative".to_string()
    };
    crate::models::MarketSummary {
        trend_direction,
        trend_strength,
        volatility_level,
        support_level,
        resistance_level,
        market_regime,
        risk_level,
        optimal_strategy,
    }
}

// Neural Network State Management Endpoints

#[derive(Debug, Serialize, Deserialize)]
pub struct NeuralState {
    pub momentum_weight: f64,
    pub rsi_weight: f64,
    pub volatility_weight: f64,
    pub pattern_weights: Vec<f64>,
    pub hidden_state: Vec<f64>,
    pub total_predictions: u64,
    pub correct_predictions: u64,
    pub learning_rate: f64,
    pub last_updated: DateTime<Utc>,
}

pub async fn store_neural_state(
    State(db): State<Arc<Database>>,
    Json(payload): Json<NeuralState>,
) -> std::result::Result<Json<ApiResponse<NeuralState>>, StatusCode> {
    match db.store_neural_state(&payload).await {
        Ok(_) => {
            let accuracy = if payload.total_predictions > 0 {
                payload.correct_predictions as f64 / payload.total_predictions as f64
            } else {
                0.0
            };
            
            info!("💾 Stored neural state: {} predictions, {:.1}% accuracy, LR: {}", 
                  payload.total_predictions, accuracy * 100.0, payload.learning_rate);
            info!("⚖️ Weights: M={:.3}, R={:.3}, V={:.3}", 
                  payload.momentum_weight, payload.rsi_weight, payload.volatility_weight);
            
            Ok(Json(ApiResponse::success(payload)))
        }
        Err(e) => {
            warn!("Failed to store neural state: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

pub async fn get_neural_state(
    State(db): State<Arc<Database>>,
) -> std::result::Result<Json<ApiResponse<Option<NeuralState>>>, StatusCode> {
    match db.get_neural_state().await {
        Ok(neural_state) => {
            if let Some(ref state) = neural_state {
                let accuracy = if state.total_predictions > 0 {
                    state.correct_predictions as f64 / state.total_predictions as f64
                } else {
                    0.0
                };
                info!("🧠 Retrieved neural state: {} predictions, {:.1}% accuracy", 
                      state.total_predictions, accuracy * 100.0);
            }
            Ok(Json(ApiResponse::success(neural_state)))
        }
        Err(e) => {
            warn!("Failed to get neural state: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

pub async fn get_neural_performance(
    State(db): State<Arc<Database>>,
) -> std::result::Result<Json<ApiResponse<serde_json::Value>>, StatusCode> {
    match db.get_neural_state().await {
        Ok(Some(neural_state)) => {
            let accuracy = if neural_state.total_predictions > 0 {
                neural_state.correct_predictions as f64 / neural_state.total_predictions as f64
            } else {
                0.5
            };
            
            // Calculate pattern confidence based on recent performance
            let pattern_confidence = (accuracy * 0.8 + 0.1).min(1.0);
            
            // Determine market regime (simplified)
            let market_regime = if accuracy > 0.7 { "Trending" } 
                              else if accuracy < 0.4 { "Volatile" } 
                              else { "Consolidating" };
            
            // Calculate risk level (inverse of accuracy)
            let risk_level = (1.0 - accuracy * 0.6).max(0.2);
            
            let performance = serde_json::json!({
                "accuracy": accuracy,
                "pattern_confidence": pattern_confidence,
                "market_regime": market_regime,
                "risk_level": risk_level,
                "total_predictions": neural_state.total_predictions,
                "learning_rate": neural_state.learning_rate,
                "neural_status": "active",
                "weights": {
                    "momentum": neural_state.momentum_weight,
                    "rsi": neural_state.rsi_weight,
                    "volatility": neural_state.volatility_weight
                },
                "last_updated": neural_state.last_updated
            });
            
            Ok(Json(ApiResponse::success(performance)))
        }
        Ok(None) => {
            // Return default values if no neural state exists
            let default_performance = serde_json::json!({
                "accuracy": 0.5,
                "pattern_confidence": 0.5,
                "market_regime": "Unknown",
                "risk_level": 0.5,
                "total_predictions": 0,
                "learning_rate": 0.01,
                "neural_status": "initializing",
                "weights": {
                    "momentum": 0.3,
                    "rsi": 0.4,
                    "volatility": 0.3
                },
                "last_updated": chrono::Utc::now()
            });
            
            Ok(Json(ApiResponse::success(default_performance)))
        }
        Err(e) => {
            warn!("Failed to get neural performance: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

pub async fn get_neural_insights(
    State(db): State<Arc<Database>>,
) -> std::result::Result<Json<ApiResponse<serde_json::Value>>, StatusCode> {
    match db.get_neural_state().await {
        Ok(Some(neural_state)) => {
            let accuracy = if neural_state.total_predictions > 0 {
                neural_state.correct_predictions as f64 / neural_state.total_predictions as f64
            } else {
                0.5
            };
            
            // Generate insights based on neural state
            let price_direction = if neural_state.momentum_weight > neural_state.rsi_weight { 0.15 } else { -0.05 };
            let volatility_forecast = (neural_state.volatility_weight * 0.7).min(1.0);
            let optimal_position_size = (accuracy * 0.8 + 0.2).min(1.0);
            
            let reasoning = format!(
                "Neural network with {:.1}% accuracy from {} predictions. {} weight dominates ({}), suggesting {} market conditions. Risk assessment: {} based on volatility weight of {:.3}.",
                accuracy * 100.0,
                neural_state.total_predictions,
                if neural_state.momentum_weight > neural_state.rsi_weight { "Momentum" } else { "RSI" },
                if neural_state.momentum_weight > neural_state.rsi_weight { neural_state.momentum_weight } else { neural_state.rsi_weight },
                if price_direction > 0.0 { "bullish" } else { "bearish" },
                if volatility_forecast > 0.5 { "elevated" } else { "moderate" },
                neural_state.volatility_weight
            );
            
            let insights = serde_json::json!({
                "price_direction": price_direction,
                "price_direction_confidence": accuracy,
                "volatility_forecast": volatility_forecast,
                "volatility_confidence": accuracy * 0.8,
                "optimal_position_size": optimal_position_size,
                "position_confidence": accuracy * 0.9,
                "reasoning": reasoning
            });
            
            Ok(Json(ApiResponse::success(insights)))
        }
        Ok(None) => {
            // Return default insights if no neural state exists
            let default_insights = serde_json::json!({
                "price_direction": 0.0,
                "price_direction_confidence": 0.5,
                "volatility_forecast": 0.5,
                "volatility_confidence": 0.5,
                "optimal_position_size": 0.5,
                "position_confidence": 0.5,
                "reasoning": "Neural network is initializing. No learned patterns available yet. Starting with default balanced approach."
            });
            
            Ok(Json(ApiResponse::success(default_insights)))
        }
        Err(e) => {
            warn!("Failed to get neural insights: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}