mod config;
mod db;
mod error;
mod handlers;
mod indicators;
mod models;

use crate::config::Config;
use crate::db::Database;
use crate::error::Result;
use crate::handlers::{
    create_wallet, get_prices, get_wallet_balances, health_check, store_balance, store_price,
    get_price_history, get_latest_price, get_technical_indicators,
    store_technical_indicators, get_latest_technical_indicators, store_trading_signal,
    get_trading_signals, get_recent_trading_signals, create_position, close_position, get_open_positions,
    get_position_history, get_all_positions, create_trading_config, get_trading_config,
    get_open_positions_by_pair, update_position_status,
    get_signals_count, get_active_positions_dashboard, get_recent_trades, get_performance_metrics,
    get_wallet_performance_metrics,
    get_candles, get_latest_candle, store_candle, get_ml_status,
    store_ml_trade_history, get_ml_trade_history, get_ml_trade_stats,
    get_advanced_indicators, get_ml_predictions, get_trading_analysis, get_market_summary,
};
use axum::{
    routing::{get, post},
    Router,
};
use dotenv::dotenv;
use std::sync::Arc;
use tower_http::cors::CorsLayer;
use tracing::{info, warn};
use tracing_subscriber;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt::init();
    
    // Load environment variables
    dotenv().ok();
    
    // Load configuration
    let config = Config::from_env()?;
    
    info!("🚀 Starting Database Service...");
    info!("═══════════════════════════════════════════════════════════════");
    info!("  🌐 Port: {}", config.port);
    info!("  💾 Database URL: {}", config.database_url);
    info!("  🔗 Max Connections: {}", config.max_connections);
    info!("═══════════════════════════════════════════════════════════════");
    
    // Initialize database
    let db = Database::new(&config.database_url, config.max_connections).await?;
    db.init_schema().await?;
    
    info!("✅ Database initialized successfully");
    
    // Create app state
    let state = Arc::new(db);
    
    // CORS layer
    let cors = CorsLayer::permissive();
    
    // Build router
    let app = Router::new()
        .route("/health", get(health_check))
        .route("/wallets", post(create_wallet))
        .route("/balances", post(store_balance))
        .route("/prices", post(store_price))
        .route("/wallets/:address/balances", get(get_wallet_balances))
        .route("/prices/:pair", get(get_prices))
        .route("/prices/:pair/history", get(get_price_history))
        .route("/prices/:pair/latest", get(get_latest_price))
        .route("/indicators/:pair", get(get_technical_indicators))
        // New enhanced routes
        .route("/indicators/:pair/store", post(store_technical_indicators))
        .route("/indicators/:pair/latest", get(get_latest_technical_indicators))
        .route("/signals", post(store_trading_signal))
        .route("/signals/:pair", get(get_trading_signals))
        .route("/signals/:pair/count", get(get_signals_count))
        .route("/trading_signals/recent", get(get_recent_trading_signals))
        .route("/positions", post(create_position))
        .route("/positions/close", post(close_position))
        .route("/positions/:address/open", get(get_open_positions))
        .route("/positions/:address/history", get(get_position_history))
        .route("/positions/all", get(get_all_positions))
        .route("/positions/pair/:pair/open", get(get_open_positions_by_pair))
        .route("/positions/:position_id/status", axum::routing::patch(update_position_status))
        .route("/positions/active", get(get_active_positions_dashboard))
        .route("/trades/recent", get(get_recent_trades))
        .route("/performance/metrics", get(get_performance_metrics))
        .route("/performance/wallets", get(get_wallet_performance_metrics))
        .route("/configs", post(create_trading_config))
        .route("/configs/:name", get(get_trading_config))
        .route("/candles/:pair/:interval", get(get_candles))
        .route("/candles/:pair/:interval/latest", get(get_latest_candle))
        .route("/candles", post(store_candle))
        .route("/ml/status", get(get_ml_status))
        .route("/ml/trades", post(store_ml_trade_history))
        .route("/ml/trades/:pair", get(get_ml_trade_history))
        .route("/ml/stats/:pair", get(get_ml_trade_stats))
        .route("/advanced_indicators/:pair", get(get_advanced_indicators))
        .route("/ml/predictions/:pair", get(get_ml_predictions))
        .route("/trading_analysis/:pair", get(get_trading_analysis))
        .route("/market_summary/:pair", get(get_market_summary))
        .layer(cors)
        .with_state(state);
    
    // Start server
    let addr = format!("0.0.0.0:{}", config.port);
    
    info!("🌐 Starting database service on {}", addr);
    
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    info!("✅ Database service listening on {}", addr);
    
    axum::serve(listener, app).await?;
    
    Ok(())
}
