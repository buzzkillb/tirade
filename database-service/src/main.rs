use anyhow::Result;
use axum::{
    extract::State,
    http::StatusCode,
    response::Json,
    routing::{get, post},
    Router,
};

use chrono::{DateTime, Utc};
use dotenv::dotenv;
use serde::{Deserialize, Serialize};
use sqlx::{sqlite::SqlitePoolOptions, SqlitePool, Row};
use std::env;
use std::sync::Arc;
use tower_http::cors::CorsLayer;
use tracing::{info, warn};
use uuid::Uuid;

// Database models
#[derive(Debug, Serialize, Deserialize)]
struct Wallet {
    id: String,
    address: String,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize)]
struct BalanceSnapshot {
    id: String,
    wallet_id: String,
    sol_balance: f64,
    usdc_balance: f64,
    timestamp: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize)]
struct PriceFeed {
    id: String,
    source: String,
    pair: String,
    price: f64,
    timestamp: DateTime<Utc>,
}

// Request/Response models
#[derive(Debug, Deserialize)]
struct CreateWalletRequest {
    address: String,
}

#[derive(Debug, Deserialize)]
struct StoreBalanceRequest {
    wallet_address: String,
    sol_balance: f64,
    usdc_balance: f64,
}

#[derive(Debug, Deserialize)]
struct StorePriceRequest {
    source: String,
    pair: String,
    price: f64,
}

#[derive(Debug, Serialize)]
struct ApiResponse<T> {
    success: bool,
    data: Option<T>,
    error: Option<String>,
}

// App state
struct AppState {
    pool: SqlitePool,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt::init();
    
    // Load environment variables
    dotenv().ok();
    
    // Database connection
    let database_url = env::var("DATABASE_URL")
        .unwrap_or_else(|_| "sqlite:./data/trading_bot.db".to_string());
    
    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await?;
    
    // Initialize database schema
    init_database(&pool).await?;
    
    // Create app state
    let state = Arc::new(AppState { pool });
    
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
    let port = env::var("PORT").unwrap_or_else(|_| "8080".to_string());
    let addr = format!("0.0.0.0:{}", port);
    
    info!("Starting database service on {}", addr);
    
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;
    
    Ok(())
}

async fn init_database(pool: &SqlitePool) -> Result<()> {
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS wallets (
            id TEXT PRIMARY KEY,
            address TEXT UNIQUE NOT NULL,
            created_at DATETIME NOT NULL,
            updated_at DATETIME NOT NULL
        )
        "#,
    )
    .execute(pool)
    .await?;
    
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS balance_snapshots (
            id TEXT PRIMARY KEY,
            wallet_id TEXT NOT NULL,
            sol_balance REAL NOT NULL,
            usdc_balance REAL NOT NULL,
            timestamp DATETIME NOT NULL,
            FOREIGN KEY (wallet_id) REFERENCES wallets (id)
        )
        "#,
    )
    .execute(pool)
    .await?;
    
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS price_feeds (
            id TEXT PRIMARY KEY,
            source TEXT NOT NULL,
            pair TEXT NOT NULL,
            price REAL NOT NULL,
            timestamp DATETIME NOT NULL
        )
        "#,
    )
    .execute(pool)
    .await?;
    
    // Create indexes for better performance
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_balance_snapshots_wallet_id ON balance_snapshots(wallet_id)")
        .execute(pool)
        .await?;
    
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_balance_snapshots_timestamp ON balance_snapshots(timestamp)")
        .execute(pool)
        .await?;
    
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_price_feeds_pair ON price_feeds(pair)")
        .execute(pool)
        .await?;
    
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_price_feeds_timestamp ON price_feeds(timestamp)")
        .execute(pool)
        .await?;
    
    info!("Database initialized successfully");
    Ok(())
}

async fn health_check() -> Json<ApiResponse<String>> {
    Json(ApiResponse {
        success: true,
        data: Some("Database service is healthy".to_string()),
        error: None,
    })
}

async fn create_wallet(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<CreateWalletRequest>,
) -> Result<Json<ApiResponse<Wallet>>, StatusCode> {
    let wallet_id = Uuid::new_v4().to_string();
    let now = Utc::now();
    
    match sqlx::query(
        r#"
        INSERT INTO wallets (id, address, created_at, updated_at)
        VALUES (?, ?, ?, ?)
        "#,
    )
    .bind(&wallet_id)
    .bind(&payload.address)
    .bind(&now)
    .bind(&now)
    .execute(&state.pool)
    .await
    {
        Ok(_) => {
            let wallet = Wallet {
                id: wallet_id,
                address: payload.address,
                created_at: now,
                updated_at: now,
            };
            info!("Created wallet: {}", wallet.address);
            Ok(Json(ApiResponse {
                success: true,
                data: Some(wallet),
                error: None,
            }))
        }
        Err(e) => {
            warn!("Failed to create wallet: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

async fn store_balance(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<StoreBalanceRequest>,
) -> Result<Json<ApiResponse<BalanceSnapshot>>, StatusCode> {
    // First, ensure wallet exists
    let wallet = match sqlx::query(
        "SELECT id, address, created_at, updated_at FROM wallets WHERE address = ?"
    )
    .bind(&payload.wallet_address)
    .fetch_optional(&state.pool)
    .await
    {
        Ok(Some(row)) => Wallet {
            id: row.try_get("id").map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?,
            address: row.try_get("address").map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?,
            created_at: row.try_get("created_at").map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?,
            updated_at: row.try_get("updated_at").map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?,
        },
        Ok(None) => {
            // Create wallet if it doesn't exist
            let wallet_id = Uuid::new_v4().to_string();
            let now = Utc::now();
            
            sqlx::query(
                r#"
                INSERT INTO wallets (id, address, created_at, updated_at)
                VALUES (?, ?, ?, ?)
                "#,
            )
            .bind(&wallet_id)
            .bind(&payload.wallet_address)
            .bind(&now)
            .bind(&now)
            .execute(&state.pool)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
            
            Wallet {
                id: wallet_id,
                address: payload.wallet_address.clone(),
                created_at: now,
                updated_at: now,
            }
        }
        Err(_) => return Err(StatusCode::INTERNAL_SERVER_ERROR),
    };
    
    // Store balance snapshot
    let snapshot_id = Uuid::new_v4().to_string();
    let now = Utc::now();
    
    match sqlx::query(
        r#"
        INSERT INTO balance_snapshots (id, wallet_id, sol_balance, usdc_balance, timestamp)
        VALUES (?, ?, ?, ?, ?)
        "#,
    )
    .bind(&snapshot_id)
    .bind(&wallet.id)
    .bind(&payload.sol_balance)
    .bind(&payload.usdc_balance)
    .bind(&now)
    .execute(&state.pool)
    .await
    {
        Ok(_) => {
            let snapshot = BalanceSnapshot {
                id: snapshot_id,
                wallet_id: wallet.id,
                sol_balance: payload.sol_balance,
                usdc_balance: payload.usdc_balance,
                timestamp: now,
            };
            info!("Stored balance for wallet {}: SOL={}, USDC={}", 
                  payload.wallet_address, payload.sol_balance, payload.usdc_balance);
            Ok(Json(ApiResponse {
                success: true,
                data: Some(snapshot),
                error: None,
            }))
        }
        Err(e) => {
            warn!("Failed to store balance: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

async fn store_price(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<StorePriceRequest>,
) -> Result<Json<ApiResponse<PriceFeed>>, StatusCode> {
    let price_id = Uuid::new_v4().to_string();
    let now = Utc::now();
    
    match sqlx::query(
        r#"
        INSERT INTO price_feeds (id, source, pair, price, timestamp)
        VALUES (?, ?, ?, ?, ?)
        "#,
    )
    .bind(&price_id)
    .bind(&payload.source)
    .bind(&payload.pair)
    .bind(&payload.price)
    .bind(&now)
    .execute(&state.pool)
    .await
    {
        Ok(_) => {
            let price_feed = PriceFeed {
                id: price_id,
                source: payload.source.clone(),
                pair: payload.pair.clone(),
                price: payload.price,
                timestamp: now,
            };
            info!("Stored price: {} {} = ${}", payload.source, payload.pair, payload.price);
            Ok(Json(ApiResponse {
                success: true,
                data: Some(price_feed),
                error: None,
            }))
        }
        Err(e) => {
            warn!("Failed to store price: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

async fn get_wallet_balances(
    State(state): State<Arc<AppState>>,
    axum::extract::Path(address): axum::extract::Path<String>,
) -> Result<Json<ApiResponse<Vec<BalanceSnapshot>>>, StatusCode> {
    let rows = sqlx::query(
        r#"
        SELECT bs.id, bs.wallet_id, bs.sol_balance, bs.usdc_balance, bs.timestamp
        FROM balance_snapshots bs
        JOIN wallets w ON bs.wallet_id = w.id
        WHERE w.address = ?
        ORDER BY bs.timestamp DESC
        LIMIT 100
        "#,
    )
    .bind(&address)
    .fetch_all(&state.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    
    let balances: Vec<BalanceSnapshot> = rows
        .into_iter()
        .map(|row| BalanceSnapshot {
            id: row.try_get("id").unwrap_or_default(),
            wallet_id: row.try_get("wallet_id").unwrap_or_default(),
            sol_balance: row.try_get("sol_balance").unwrap_or_default(),
            usdc_balance: row.try_get("usdc_balance").unwrap_or_default(),
            timestamp: row.try_get("timestamp").unwrap_or_default(),
        })
        .collect();
    
    Ok(Json(ApiResponse {
        success: true,
        data: Some(balances),
        error: None,
    }))
}

async fn get_prices(
    State(state): State<Arc<AppState>>,
    axum::extract::Path(pair): axum::extract::Path<String>,
) -> Result<Json<ApiResponse<Vec<PriceFeed>>>, StatusCode> {
    let rows = sqlx::query(
        r#"
        SELECT id, source, pair, price, timestamp
        FROM price_feeds
        WHERE pair = ?
        ORDER BY timestamp DESC
        LIMIT 100
        "#,
    )
    .bind(&pair)
    .fetch_all(&state.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    
    let prices: Vec<PriceFeed> = rows
        .into_iter()
        .map(|row| PriceFeed {
            id: row.try_get("id").unwrap_or_default(),
            source: row.try_get("source").unwrap_or_default(),
            pair: row.try_get("pair").unwrap_or_default(),
            price: row.try_get("price").unwrap_or_default(),
            timestamp: row.try_get("timestamp").unwrap_or_default(),
        })
        .collect();
    
    Ok(Json(ApiResponse {
        success: true,
        data: Some(prices),
        error: None,
    }))
}
