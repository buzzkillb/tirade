mod config;
mod db;
mod error;
mod handlers;
mod models;

use crate::config::Config;
use crate::db::Database;
use crate::error::Result;
use crate::handlers::{
    create_wallet, get_prices, get_wallet_balances, health_check, store_balance, store_price,
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
    
    // Initialize database
    let db = Database::new(&config.database_url, config.max_connections).await?;
    db.init_schema().await?;
    
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
        .layer(cors)
        .with_state(state);
    
    // Start server
    let addr = format!("0.0.0.0:{}", config.port);
    
    info!("Starting database service on {}", addr);
    
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;
    
    Ok(())
}
