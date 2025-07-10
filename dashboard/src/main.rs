use actix_web::{web, App, HttpServer, HttpResponse, Result, Error};
use actix_files::Files;
use actix_web::web::Bytes;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use chrono::{DateTime, Utc};
use reqwest;
use futures::stream::Stream;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Duration;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PriceData {
    pub id: String,
    pub source: String,
    pub pair: String,
    pub price: f64,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TechnicalIndicator {
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

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TradingSignal {
    pub id: String,
    pub pair: String,
    pub signal_type: String,
    pub confidence: f64,
    pub price: f64,
    pub timestamp: DateTime<Utc>,
    pub reasoning: String,
    pub take_profit: Option<f64>,
    pub stop_loss: Option<f64>,
    pub executed: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Position {
    pub id: String,
    pub pair: String,
    pub position_type: String,
    pub entry_price: f64,
    pub current_price: f64,
    pub quantity: f64,
    pub pnl: f64,
    pub pnl_percent: f64,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
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

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PerformanceMetrics {
    pub total_trades: i64,
    pub winning_trades: i64,
    pub losing_trades: i64,
    pub win_rate: f64,
    pub total_pnl: f64,
    pub total_pnl_percent: f64,
    pub avg_trade_pnl: f64,
    pub max_drawdown: f64,
    pub sharpe_ratio: f64,
    pub total_volume: f64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SystemStatus {
    pub database_connected: bool,
    pub price_feed_running: bool,
    pub trading_logic_running: bool,
    pub trading_execution_enabled: bool,
    pub last_price_update: Option<DateTime<Utc>>,
    pub last_signal_generated: Option<DateTime<Utc>>,
    pub active_position: bool,
    pub total_signals_today: i64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DashboardData {
    pub system_status: SystemStatus,
    pub latest_prices: Vec<PriceData>,
    pub latest_indicators: Vec<TechnicalIndicator>,
    pub latest_signals: Vec<TradingSignal>,
    pub active_positions: Vec<Position>,
    pub recent_trades: Vec<Trade>,
    pub performance: PerformanceMetrics,
    pub price_history: Vec<PriceData>,
    pub market_sentiment: String,
    pub price_changes: PriceChanges,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PriceChanges {
    pub change_1h: Option<f64>,
    pub change_4h: Option<f64>,
    pub change_12h: Option<f64>,
    pub change_24h: Option<f64>,
}

#[derive(Debug, Clone)]
struct CachedData {
    data: DashboardData,
    timestamp: DateTime<Utc>,
    ttl_seconds: i64,
}

impl CachedData {
    fn is_expired(&self) -> bool {
        let now = Utc::now();
        (now - self.timestamp).num_seconds() > self.ttl_seconds
    }
}

struct AppState {
    database_url: String,
    cache: Arc<Mutex<HashMap<String, CachedData>>>,
    client: reqwest::Client,
}

async fn get_dashboard_data(state: web::Data<AppState>) -> Result<HttpResponse> {
    // Check cache first
    const CACHE_TTL_SECONDS: i64 = 2; // Cache for 2 seconds for more real-time updates
    let cache_key = "dashboard_data";
    
    {
        let cache = state.cache.lock().await;
        if let Some(cached) = cache.get(cache_key) {
            if !cached.is_expired() {
                return Ok(HttpResponse::Ok().json(cached.data.clone()));
            }
        }
    }

    // Fetch fresh data
    let dashboard_data = fetch_fresh_dashboard_data(&state).await;
    
    // Cache the result
    {
        let mut cache = state.cache.lock().await;
        cache.insert(cache_key.to_string(), CachedData {
            data: dashboard_data.clone(),
            timestamp: Utc::now(),
            ttl_seconds: CACHE_TTL_SECONDS,
        });
    }

    Ok(HttpResponse::Ok().json(dashboard_data))
}

// Real-time trading signals endpoint (no cache)
async fn get_realtime_signals(state: web::Data<AppState>) -> Result<HttpResponse> {
    let signals = fetch_signals(&state.client, &state.database_url).await;
    
    match signals {
        Ok(signals) => Ok(HttpResponse::Ok().json(signals)),
        Err(e) => {
            eprintln!("Error fetching real-time signals: {}", e);
            Ok(HttpResponse::InternalServerError().json(vec![] as Vec<TradingSignal>))
        }
    }
}

async fn fetch_fresh_dashboard_data(state: &web::Data<AppState>) -> DashboardData {
    let mut dashboard_data = DashboardData {
        system_status: SystemStatus {
            database_connected: false,
            price_feed_running: false,
            trading_logic_running: false,
            trading_execution_enabled: false,
            last_price_update: None,
            last_signal_generated: None,
            active_position: false,
            total_signals_today: 0,
        },
        latest_prices: Vec::new(),
        latest_indicators: Vec::new(),
        latest_signals: Vec::new(),
        active_positions: Vec::new(),
        recent_trades: Vec::new(),
        performance: PerformanceMetrics {
            total_trades: 0,
            winning_trades: 0,
            losing_trades: 0,
            win_rate: 0.0,
            total_pnl: 0.0,
            total_pnl_percent: 0.0,
            avg_trade_pnl: 0.0,
            max_drawdown: 0.0,
            sharpe_ratio: 0.0,
            total_volume: 0.0,
        },
        price_history: Vec::new(),
        market_sentiment: "Neutral".to_string(),
        price_changes: PriceChanges {
            change_1h: None,
            change_4h: None,
            change_12h: None,
            change_24h: None,
        },
    };

    // Make all API calls in parallel for better performance
    let (
        pyth_price_result,
        jupiter_price_result,
        coinbase_price_result,
        indicators_result,
        signals_result,
        positions_result,
        trades_result,
        performance_result,
        price_history_result,
        health_result,
        signals_count_result,
    ) = tokio::join!(
        fetch_pyth_price(&state.client, &state.database_url),
        fetch_jupiter_price(&state.client, &state.database_url),
        fetch_coinbase_price_direct(&state.client), // Direct Coinbase API call for visual reference
        fetch_indicators(&state.client, &state.database_url),
        fetch_signals(&state.client, &state.database_url),
        fetch_positions(&state.client, &state.database_url),
        fetch_trades(&state.client, &state.database_url),
        fetch_performance(&state.client, &state.database_url),
        fetch_price_history(&state.client, &state.database_url),
        fetch_health(&state.client, &state.database_url),
        fetch_signals_count(&state.client, &state.database_url),
    );

    // Process all price data sources
    let mut all_prices = Vec::new();
    let mut latest_timestamp = None;
    
    if let Ok(Some(price)) = pyth_price_result {
        all_prices.push(price.clone());
        if latest_timestamp.is_none() || price.timestamp > latest_timestamp.unwrap() {
            latest_timestamp = Some(price.timestamp);
        }
    }
    
    if let Ok(Some(price)) = jupiter_price_result {
        all_prices.push(price.clone());
        if latest_timestamp.is_none() || price.timestamp > latest_timestamp.unwrap() {
            latest_timestamp = Some(price.timestamp);
        }
    }

    // Add Coinbase price for visual reference (not stored in database)
    if let Ok(Some(price)) = coinbase_price_result {
        all_prices.push(price.clone());
        if latest_timestamp.is_none() || price.timestamp > latest_timestamp.unwrap() {
            latest_timestamp = Some(price.timestamp);
        }
    }
    
    dashboard_data.latest_prices = all_prices;
    dashboard_data.system_status.last_price_update = latest_timestamp;
    
    // Check if price feed is running (any source updated within 5 minutes)
    if let Some(timestamp) = latest_timestamp {
        let five_minutes_ago = Utc::now() - chrono::Duration::minutes(5);
        dashboard_data.system_status.price_feed_running = timestamp > five_minutes_ago;
    }

    // Process indicators
    if let Ok(Some(indicator)) = indicators_result {
        dashboard_data.latest_indicators = vec![indicator];
    }

    // Process signals
    if let Ok(signals) = signals_result {
        // Calculate market sentiment efficiently
        let four_hours_ago = Utc::now() - chrono::Duration::hours(4);
        let (bullish_count, bearish_count) = signals.iter()
            .filter(|s| s.timestamp > four_hours_ago)
            .fold((0, 0), |(bull, bear), s| {
                match s.signal_type.to_lowercase().as_str() {
                    "buy" => (bull + 1, bear),
                    "sell" => (bull, bear + 1),
                    _ => (bull, bear),
                }
            });
        
        dashboard_data.market_sentiment = match bullish_count.cmp(&bearish_count) {
            std::cmp::Ordering::Greater => format!("🐂 Bullish ({} bullish, {} bearish signals)", bullish_count, bearish_count),
            std::cmp::Ordering::Less => format!("🐻 Bearish ({} bullish, {} bearish signals)", bullish_count, bearish_count),
            std::cmp::Ordering::Equal => format!("⚖️ Neutral ({} bullish, {} bearish signals)", bullish_count, bearish_count),
        };
        
        dashboard_data.latest_signals = signals;
        if let Some(latest_signal) = dashboard_data.latest_signals.first() {
            dashboard_data.system_status.last_signal_generated = Some(latest_signal.timestamp);
            let ten_minutes_ago = Utc::now() - chrono::Duration::minutes(10);
            dashboard_data.system_status.trading_logic_running = latest_signal.timestamp > ten_minutes_ago;
        }
    }

    // Process positions
    if let Ok(positions) = positions_result {
        dashboard_data.system_status.active_position = !positions.is_empty();
        dashboard_data.active_positions = positions;
    }

    // Process trades
    if let Ok(trades) = trades_result {
        dashboard_data.recent_trades = trades;
    }

    // Process performance
    if let Ok(performance) = performance_result {
        dashboard_data.performance = performance;
    }

    // Process price history and calculate changes efficiently
    if let Ok(mut price_history) = price_history_result {
        if !price_history.is_empty() {
            // Sort once, in place
            price_history.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
            
            let current_price = price_history[0].price;
            let now = Utc::now();
            
            // Find prices at different intervals efficiently
            let target_times = [
                (now - chrono::Duration::hours(1), "1h"),
                (now - chrono::Duration::hours(4), "4h"),
                (now - chrono::Duration::hours(12), "12h"),
                (now - chrono::Duration::hours(24), "24h"),
            ];
            
            let mut changes = [None; 4];
            let mut target_idx = 0;
            
            for price in &price_history {
                while target_idx < target_times.len() && price.timestamp <= target_times[target_idx].0 {
                    if price.price > 0.0 && price.price != current_price {
                        changes[target_idx] = Some(((current_price - price.price) / price.price) * 100.0);
                    }
                    target_idx += 1;
                }
                if target_idx >= target_times.len() {
                    break;
                }
            }
            
            dashboard_data.price_changes = PriceChanges {
                change_1h: changes[0],
                change_4h: changes[1],
                change_12h: changes[2],
                change_24h: changes[3],
            };
            
            dashboard_data.price_history = price_history;
        }
    }

    // Process health check
    dashboard_data.system_status.database_connected = health_result.unwrap_or(false);

    // Check trading execution status
    let trading_execution_enabled = std::env::var("ENABLE_TRADING_EXECUTION")
        .unwrap_or_else(|_| "false".to_string())
        .parse::<bool>()
        .unwrap_or(false);
    dashboard_data.system_status.trading_execution_enabled = trading_execution_enabled;

    // Process signals count
    dashboard_data.system_status.total_signals_today = signals_count_result.unwrap_or(0);

    dashboard_data
}

// Optimized parallel fetch functions
async fn fetch_pyth_price(client: &reqwest::Client, database_url: &str) -> Result<Option<PriceData>, reqwest::Error> {
    let response = client
        .get(&format!("{}/prices/SOL%2FUSDC/latest?source=pyth", database_url))
        .timeout(Duration::from_secs(5))
        .send()
        .await?;
    
    if let Ok(api_response) = response.json::<serde_json::Value>().await {
        if let Some(price_data) = api_response["data"].as_object() {
            if let Ok(price) = serde_json::from_value::<PriceData>(serde_json::Value::Object(price_data.clone())) {
                return Ok(Some(price));
            }
        }
    }
    Ok(None)
}

async fn fetch_jupiter_price(client: &reqwest::Client, database_url: &str) -> Result<Option<PriceData>, reqwest::Error> {
    let response = client
        .get(&format!("{}/prices/SOL%2FUSDC/latest?source=jupiter", database_url))
        .timeout(Duration::from_secs(5))
        .send()
        .await?;
    
    if let Ok(api_response) = response.json::<serde_json::Value>().await {
        if let Some(price_data) = api_response["data"].as_object() {
            if let Ok(price) = serde_json::from_value::<PriceData>(serde_json::Value::Object(price_data.clone())) {
                return Ok(Some(price));
            }
        }
    }
    Ok(None)
}

async fn fetch_coinbase_price_direct(client: &reqwest::Client) -> Result<Option<PriceData>, reqwest::Error> {
    let response = client
        .get("https://api.coinbase.com/v2/prices/SOL-USD/spot")
        .timeout(Duration::from_secs(3))
        .send()
        .await?;
    
    if response.status().is_success() {
        if let Ok(api_response) = response.json::<serde_json::Value>().await {
            if let Some(price_str) = api_response["data"]["amount"].as_str() {
                if let Ok(price_float) = price_str.parse::<f64>() {
                    let price_data = PriceData {
                        id: format!("coinbase-{}", chrono::Utc::now().timestamp()),
                        source: "coinbase".to_string(),
                        pair: "SOL/USD".to_string(),
                        price: price_float,
                        timestamp: chrono::Utc::now(),
                    };
                    return Ok(Some(price_data));
                }
            }
        }
    }
    Ok(None)
}

async fn fetch_indicators(client: &reqwest::Client, database_url: &str) -> Result<Option<TechnicalIndicator>, reqwest::Error> {
    let response = client
        .get(&format!("{}/indicators/SOL%2FUSDC", database_url))
        .timeout(Duration::from_secs(5))
        .send()
        .await?;
    
    if let Ok(api_response) = response.json::<serde_json::Value>().await {
        if let Some(indicator_data) = api_response["data"].as_object() {
            let mut indicator = TechnicalIndicator {
                id: "latest".to_string(),
                pair: indicator_data["pair"].as_str().unwrap_or("SOL/USDC").to_string(),
                timestamp: Utc::now(),
                sma_20: indicator_data["sma_20"].as_f64(),
                sma_50: indicator_data["sma_50"].as_f64(),
                sma_200: indicator_data["sma_200"].as_f64(),
                rsi_14: indicator_data["rsi_14"].as_f64(),
                price_change_24h: indicator_data["price_change_24h"].as_f64(),
                price_change_percent_24h: indicator_data["price_change_percent_24h"].as_f64(),
                volatility_24h: indicator_data["volatility_24h"].as_f64(),
                current_price: indicator_data["current_price"].as_f64().unwrap_or(0.0),
                created_at: Utc::now(),
            };
            
            if let Some(timestamp_str) = indicator_data["timestamp"].as_str() {
                if let Ok(timestamp) = chrono::DateTime::parse_from_rfc3339(timestamp_str) {
                    indicator.timestamp = timestamp.with_timezone(&chrono::Utc);
                }
            }
            
            return Ok(Some(indicator));
        }
    }
    Ok(None)
}

async fn fetch_signals(client: &reqwest::Client, database_url: &str) -> Result<Vec<TradingSignal>, reqwest::Error> {
    let response = client
        .get(&format!("{}/signals/SOL%2FUSDC", database_url))
        .timeout(Duration::from_secs(5))
        .send()
        .await?;
    
    if let Ok(api_response) = response.json::<serde_json::Value>().await {
        if let Some(signals_array) = api_response["data"].as_array() {
            if let Ok(signals) = serde_json::from_value::<Vec<TradingSignal>>(serde_json::Value::Array(signals_array.clone())) {
                return Ok(signals);
            }
        }
    }
    Ok(Vec::new())
}

async fn fetch_positions(client: &reqwest::Client, database_url: &str) -> Result<Vec<Position>, reqwest::Error> {
    let response = client
        .get(&format!("{}/positions/active", database_url))
        .timeout(Duration::from_secs(5))
        .send()
        .await?;
    
    if let Ok(api_response) = response.json::<serde_json::Value>().await {
        if let Some(positions_array) = api_response["data"].as_array() {
            if let Ok(positions) = serde_json::from_value::<Vec<Position>>(serde_json::Value::Array(positions_array.clone())) {
                return Ok(positions);
            }
        }
    }
    Ok(Vec::new())
}

async fn fetch_trades(client: &reqwest::Client, database_url: &str) -> Result<Vec<Trade>, reqwest::Error> {
    let response = client
        .get(&format!("{}/trades/recent", database_url))
        .timeout(Duration::from_secs(5))
        .send()
        .await?;
    
    if let Ok(api_response) = response.json::<serde_json::Value>().await {
        if let Some(trades_array) = api_response["data"].as_array() {
            if let Ok(trades) = serde_json::from_value::<Vec<Trade>>(serde_json::Value::Array(trades_array.clone())) {
                return Ok(trades);
            }
        }
    }
    Ok(Vec::new())
}

async fn fetch_performance(client: &reqwest::Client, database_url: &str) -> Result<PerformanceMetrics, reqwest::Error> {
    let response = client
        .get(&format!("{}/performance/metrics", database_url))
        .timeout(Duration::from_secs(5))
        .send()
        .await?;
    
    if let Ok(performance) = response.json::<PerformanceMetrics>().await {
        return Ok(performance);
    }
    
    Ok(PerformanceMetrics {
        total_trades: 0,
        winning_trades: 0,
        losing_trades: 0,
        win_rate: 0.0,
        total_pnl: 0.0,
        total_pnl_percent: 0.0,
        avg_trade_pnl: 0.0,
        max_drawdown: 0.0,
        sharpe_ratio: 0.0,
        total_volume: 0.0,
    })
}

async fn fetch_price_history(client: &reqwest::Client, database_url: &str) -> Result<Vec<PriceData>, reqwest::Error> {
    // Reduced from 48 hours to 12 hours for better performance
    let response = client
        .get(&format!("{}/prices/SOL%2FUSDC/history?hours=12", database_url))
        .timeout(Duration::from_secs(10))
        .send()
        .await?;
    
    if let Ok(api_response) = response.json::<serde_json::Value>().await {
        if let Some(price_history_array) = api_response["data"].as_array() {
            if let Ok(price_history) = serde_json::from_value::<Vec<PriceData>>(serde_json::Value::Array(price_history_array.clone())) {
                return Ok(price_history);
            }
        }
    }
    Ok(Vec::new())
}

async fn fetch_health(client: &reqwest::Client, database_url: &str) -> Result<bool, reqwest::Error> {
    let response = client
        .get(&format!("{}/health", database_url))
        .timeout(Duration::from_secs(3))
        .send()
        .await?;
    Ok(response.status().is_success())
}

async fn fetch_signals_count(client: &reqwest::Client, database_url: &str) -> Result<i64, reqwest::Error> {
    let response = client
        .get(&format!("{}/signals/SOL%2FUSDC/count?hours=24", database_url))
        .timeout(Duration::from_secs(5))
        .send()
        .await?;
    
    if let Ok(count_response) = response.json::<serde_json::Value>().await {
        if let Some(count) = count_response["data"]["count"].as_i64() {
            return Ok(count);
        }
    }
    Ok(0)
}

struct DashboardStream {
    interval: tokio::time::Interval,
}

struct PriceStream {
    interval: tokio::time::Interval,
}

impl Stream for DashboardStream {
    type Item = Result<Bytes, Error>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        if self.interval.poll_tick(cx).is_ready() {
            // For real-time trading signals, we send an update notification
            let data = serde_json::json!({
                "type": "dashboard_update",
                "timestamp": chrono::Utc::now().to_rfc3339(),
                "message": "Trading signals updated - fetch latest data"
            });
            
            let json_str = data.to_string();
            let bytes = format!("data: {}\n\n", json_str);
            
            Poll::Ready(Some(Ok(Bytes::from(bytes))))
        } else {
            Poll::Pending
        }
    }
}

impl Stream for PriceStream {
    type Item = Result<Bytes, Error>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        if self.interval.poll_tick(cx).is_ready() {
            // Real-time price updates every second (like coinbase)
            let data = serde_json::json!({
                "type": "price_update",
                "timestamp": chrono::Utc::now().to_rfc3339(),
                "message": "Price updated - fetch latest from /api/dashboard"
            });
            
            let json_str = data.to_string();
            let bytes = format!("data: {}\n\n", json_str);
            
            Poll::Ready(Some(Ok(Bytes::from(bytes))))
        } else {
            Poll::Pending
        }
    }
}

async fn dashboard_stream(_state: web::Data<AppState>) -> Result<HttpResponse> {
    let stream = DashboardStream {
        interval: tokio::time::interval(tokio::time::Duration::from_secs(15)), // Trading signals every 15 seconds
    };

    Ok(HttpResponse::Ok()
        .content_type("text/event-stream")
        .header("Cache-Control", "no-cache")
        .header("Connection", "keep-alive")
        .header("Access-Control-Allow-Origin", "*")
        .streaming(stream))
}

async fn price_stream(_state: web::Data<AppState>) -> Result<HttpResponse> {
    let stream = PriceStream {
        interval: tokio::time::interval(tokio::time::Duration::from_secs(1)), // Keep 1-second price updates
    };

    Ok(HttpResponse::Ok()
        .content_type("text/event-stream")
        .header("Cache-Control", "no-cache")
        .header("Connection", "keep-alive")
        .header("Access-Control-Allow-Origin", "*")
        .streaming(stream))
}

async fn index() -> Result<HttpResponse> {
    let html = r#"
<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>TiRADE Dashboard</title>
    <link rel="icon" href="data:image/svg+xml,<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 100 100'><defs><linearGradient id='solana' x1='0%' y1='0%' x2='100%' y2='100%'><stop offset='0%' style='stop-color:%239945ff;stop-opacity:1' /><stop offset='50%' style='stop-color:%2314f195;stop-opacity:1' /><stop offset='100%' style='stop-color:%239945ff;stop-opacity:1' /></linearGradient></defs><rect width='100' height='100' rx='20' fill='url(%23solana)'/><text x='50' y='65' font-family='Arial, sans-serif' font-size='50' font-weight='bold' text-anchor='middle' fill='white'>T</text></svg>">

    <style>
        * {
            margin: 0;
            padding: 0;
            box-sizing: border-box;
        }

        body {
            font-family: 'Segoe UI', Tahoma, Geneva, Verdana, sans-serif;
            background: linear-gradient(135deg, 
                #000000 0%, 
                #0a0a0a 15%, 
                #1a0a2e 30%, 
                #16213e 50%, 
                #0f3460 70%, 
                #533483 85%, 
                #000000 100%);
            color: #ffffff;
            min-height: 100vh;
            position: relative;
        }

        body::before {
            content: '';
            position: fixed;
            top: 0;
            left: 0;
            width: 100%;
            height: 100%;
            background: radial-gradient(circle at 20% 80%, rgba(153, 69, 255, 0.15) 0%, transparent 50%),
                        radial-gradient(circle at 80% 20%, rgba(20, 241, 149, 0.1) 0%, transparent 50%),
                        radial-gradient(circle at 40% 40%, rgba(153, 69, 255, 0.05) 0%, transparent 50%);
            pointer-events: none;
            z-index: -1;
        }

        .container {
            max-width: 1400px;
            margin: 0 auto;
            padding: 20px;
        }

        .header {
            text-align: center;
            margin-bottom: 30px;
            color: #9945ff;
        }

        .header h1 {
            font-size: 2.5rem;
            margin-bottom: 10px;
            text-shadow: 0 0 30px rgba(153, 69, 255, 0.7);
            background: linear-gradient(45deg, #9945ff, #14f195);
            -webkit-background-clip: text;
            -webkit-text-fill-color: transparent;
            background-clip: text;
        }

        .sol-logo {
            display: inline-block;
            background: linear-gradient(45deg, #9945ff, #14f195, #9945ff);
            background-size: 200% 200%;
            -webkit-background-clip: text;
            -webkit-text-fill-color: transparent;
            background-clip: text;
            font-weight: bold;
            font-size: 1.2em;
            text-shadow: 0 0 20px rgba(153, 69, 255, 0.8);
            animation: solGlow 3s ease-in-out infinite;
            position: relative;
        }

        .sol-logo::before {
            content: '';
            position: absolute;
            top: -2px;
            left: -2px;
            right: -2px;
            bottom: -2px;
            background: linear-gradient(45deg, #9945ff, #14f195, #9945ff);
            background-size: 200% 200%;
            z-index: -1;
            border-radius: 8px;
            opacity: 0.3;
            animation: solGlow 3s ease-in-out infinite;
        }

        @keyframes solGlow {
            0%, 100% {
                background-position: 0% 50%;
                filter: brightness(1);
            }
            50% {
                background-position: 100% 50%;
                filter: brightness(1.2);
            }
        }

        @keyframes slideIn {
            from {
                transform: translateX(100%);
                opacity: 0;
            }
            to {
                transform: translateX(0);
                opacity: 1;
            }
        }

        @keyframes pulse {
            0%, 100% {
                opacity: 1;
            }
            50% {
                opacity: 0.7;
            }
        }

        .header p {
            font-size: 1.1rem;
            opacity: 0.7;
            color: #888888;
        }

        .grid {
            display: grid;
            grid-template-columns: repeat(auto-fit, minmax(300px, 1fr));
            gap: 20px;
            margin-bottom: 30px;
        }

        .card {
            background: linear-gradient(145deg, #0a0a0a, #1a1a1a);
            border: 1px solid #333333;
            border-radius: 15px;
            padding: 20px;
            box-shadow: 0 10px 30px rgba(0,0,0,0.8);
            transition: all 0.3s ease;
            position: relative;
            overflow: hidden;
        }

        .card::before {
            content: '';
            position: absolute;
            top: 0;
            left: 0;
            right: 0;
            height: 2px;
            background: linear-gradient(90deg, #9945ff, #14f195);
        }

        .card:hover {
            transform: translateY(-5px);
            box-shadow: 0 15px 40px rgba(153, 69, 255, 0.3);
            border-color: #9945ff;
        }

        .card h3 {
            color: #9945ff;
            margin-bottom: 15px;
            font-size: 1.2rem;
            border-bottom: 2px solid #333333;
            padding-bottom: 10px;
            text-shadow: 0 0 10px rgba(153, 69, 255, 0.5);
        }

        .status-grid {
            display: grid;
            grid-template-columns: repeat(auto-fit, minmax(200px, 1fr));
            gap: 15px;
        }

        .status-item {
            text-align: center;
            padding: 15px;
            border-radius: 10px;
            background: linear-gradient(145deg, #0a0a0a, #1a1a1a);
            border: 1px solid #333333;
            color: #ffffff;
        }

        .status-item.connected {
            background: linear-gradient(145deg, #0d2b1a, #1a2e1a);
            border-color: #14f195;
            color: #14f195;
        }

        .status-item.disconnected {
            background: linear-gradient(145deg, #2b1a1a, #2e1a1a);
            border-color: #ff6b6b;
            color: #ff6b6b;
        }

        .metric {
            display: flex;
            justify-content: space-between;
            align-items: center;
            padding: 10px 0;
            border-bottom: 1px solid #333333;
        }

        .metric:last-child {
            border-bottom: none;
        }

        .metric-value {
            font-weight: bold;
            font-size: 1.1rem;
        }

        .positive { color: #14f195; text-shadow: 0 0 5px rgba(20, 241, 149, 0.5); }
        .negative { color: #ff6b6b; text-shadow: 0 0 5px rgba(255, 107, 107, 0.5); }
        .neutral { color: #888888; }

        .price-chart {
            grid-column: 1 / -1;
            height: 400px;
            position: relative;
        }
        


        .signal-item {
            padding: 10px;
            margin: 5px 0;
            border-radius: 8px;
            border-left: 4px solid;
            background: linear-gradient(145deg, #0a0a0a, #1a1a1a);
            border: 1px solid #333333;
        }

        .signal-buy { 
            background: linear-gradient(145deg, #1a0a2e, #16213e, #0f3460); 
            border-left-color: #14f195; 
            border-color: #14f195;
            box-shadow: 0 0 15px rgba(20, 241, 149, 0.2);
            position: relative;
            overflow: hidden;
        }

        .signal-buy::before {
            content: '';
            position: absolute;
            top: 0;
            left: 0;
            right: 0;
            bottom: 0;
            background: linear-gradient(45deg, 
                rgba(20, 241, 149, 0.08) 0%, 
                rgba(153, 69, 255, 0.03) 50%, 
                rgba(20, 241, 149, 0.08) 100%);
            pointer-events: none;
        }

        .signal-sell { 
            background: linear-gradient(145deg, #1a0a2e, #16213e, #0f3460); 
            border-left-color: #ff6b6b; 
            border-color: #ff6b6b;
            box-shadow: 0 0 15px rgba(255, 107, 107, 0.2);
            position: relative;
            overflow: hidden;
        }

        .signal-sell::before {
            content: '';
            position: absolute;
            top: 0;
            left: 0;
            right: 0;
            bottom: 0;
            background: linear-gradient(45deg, 
                rgba(255, 107, 107, 0.08) 0%, 
                rgba(153, 69, 255, 0.03) 50%, 
                rgba(255, 107, 107, 0.08) 100%);
            pointer-events: none;
        }
        .signal-hold { 
            background: linear-gradient(145deg, #0a0514, #0d0f1a, #061220); 
            border-left-color: #9945ff; 
            border-color: #9945ff;
            box-shadow: 0 0 15px rgba(153, 69, 255, 0.2);
            position: relative;
            overflow: hidden;
        }

        .signal-hold::before {
            content: '';
            position: absolute;
            top: 0;
            left: 0;
            right: 0;
            bottom: 0;
            background: linear-gradient(45deg, 
                rgba(153, 69, 255, 0.08) 0%, 
                rgba(20, 241, 149, 0.03) 50%, 
                rgba(153, 69, 255, 0.08) 100%);
            pointer-events: none;
        }

        .market-sentiment {
            margin-bottom: 15px;
            padding: 12px;
            border-radius: 10px;
            background: linear-gradient(145deg, #0a0a0a, #1a1a1a);
            border: 1px solid #333333;
            text-align: center;
            position: relative;
            overflow: hidden;
        }

        .market-sentiment::before {
            content: '';
            position: absolute;
            top: 0;
            left: 0;
            right: 0;
            height: 2px;
            background: linear-gradient(90deg, #9945ff, #14f195);
        }

        .sentiment-indicator {
            display: flex;
            justify-content: center;
            align-items: center;
            gap: 10px;
        }

        .sentiment-text {
            font-size: 1.1rem;
            font-weight: bold;
            color: #ffffff;
            text-shadow: 0 0 10px rgba(153, 69, 255, 0.5);
            animation: pulse 2s ease-in-out infinite;
        }

        .sentiment-text:contains('🐂') {
            color: #14f195;
            text-shadow: 0 0 10px rgba(20, 241, 149, 0.5);
        }

        .sentiment-text:contains('🐻') {
            color: #ff6b6b;
            text-shadow: 0 0 10px rgba(255, 107, 107, 0.5);
        }
        
        /* New signal animations */
        .signal-item.new-signal {
            animation: newSignalPulse 2s ease-in-out;
            position: relative;
            overflow: hidden;
        }
        
        .signal-item.new-signal::before {
            content: '';
            position: absolute;
            top: 0;
            left: -100%;
            width: 100%;
            height: 100%;
            background: linear-gradient(90deg, transparent, rgba(255, 255, 255, 0.2), transparent);
            animation: signalShine 1.5s ease-in-out;
        }
        
        .signal-item.new-signal::after {
            content: '🆕';
            position: absolute;
            top: 10px;
            right: 10px;
            font-size: 1.2rem;
            animation: newBadgeBounce 1s ease-in-out infinite;
        }
        
        @keyframes newSignalPulse {
            0% {
                transform: scale(1);
                box-shadow: 0 0 0 0 rgba(153, 69, 255, 0.7);
            }
            50% {
                transform: scale(1.02);
                box-shadow: 0 0 0 10px rgba(153, 69, 255, 0);
            }
            100% {
                transform: scale(1);
                box-shadow: 0 0 0 0 rgba(153, 69, 255, 0);
            }
        }
        
        @keyframes signalShine {
            0% {
                left: -100%;
            }
            100% {
                left: 100%;
            }
        }
        
        @keyframes newBadgeBounce {
            0%, 100% {
                transform: scale(1);
            }
            50% {
                transform: scale(1.2);
            }
        }
        
        /* Signal type specific animations */
        .signal-buy.new-signal {
            animation: newSignalPulse 2s ease-in-out, buySignalGlow 3s ease-in-out infinite;
        }
        
        .signal-sell.new-signal {
            animation: newSignalPulse 2s ease-in-out, sellSignalGlow 3s ease-in-out infinite;
        }
        
        .signal-hold.new-signal {
            animation: newSignalPulse 2s ease-in-out, holdSignalGlow 3s ease-in-out infinite;
        }

        /* Signal Triggers Styles */
        .signal-triggers {
            margin: 10px 0;
            padding: 8px;
            background: rgba(0, 0, 0, 0.3);
            border-radius: 6px;
            border: 1px solid rgba(153, 69, 255, 0.2);
        }

        .triggers-label {
            font-size: 0.85rem;
            font-weight: bold;
            color: #9945ff;
            margin-bottom: 8px;
            text-transform: uppercase;
            letter-spacing: 0.5px;
        }

        .triggers-grid {
            display: grid;
            grid-template-columns: repeat(auto-fit, minmax(140px, 1fr));
            gap: 6px;
        }

        .trigger-checkbox {
            display: flex;
            align-items: center;
            gap: 6px;
            font-size: 0.75rem;
            color: #888;
            padding: 4px 6px;
            border-radius: 4px;
            background: rgba(0, 0, 0, 0.2);
            border: 1px solid rgba(255, 255, 255, 0.1);
            cursor: default;
            transition: all 0.2s ease;
        }

        .trigger-checkbox.active {
            background: rgba(153, 69, 255, 0.15);
            border-color: rgba(153, 69, 255, 0.4);
            color: #fff;
            box-shadow: 0 0 8px rgba(153, 69, 255, 0.2);
        }

        .trigger-checkbox input[type="checkbox"] {
            width: 12px;
            height: 12px;
            accent-color: #9945ff;
            cursor: default;
        }

        .trigger-checkbox.active input[type="checkbox"] {
            accent-color: #14f195;
        }

        .trigger-checkbox span {
            font-weight: 500;
        }

        /* Signal Reasoning Styles */
        .signal-reasoning {
            margin: 10px 0;
            padding: 8px;
            background: rgba(0, 0, 0, 0.2);
            border-radius: 6px;
            border: 1px solid rgba(255, 255, 255, 0.1);
        }

        .reasoning-label {
            font-size: 0.85rem;
            font-weight: bold;
            color: #14f195;
            margin-bottom: 6px;
            text-transform: uppercase;
            letter-spacing: 0.5px;
        }

        .reasoning-text {
            font-size: 0.8rem;
            color: #ffffff;
            line-height: 1.4;
            font-family: 'Courier New', monospace;
        }

        .reasoning-item {
            margin: 4px 0;
            padding: 4px 8px;
            background: rgba(153, 69, 255, 0.1);
            border-radius: 4px;
            border-left: 3px solid #9945ff;
            font-size: 0.75rem;
            color: #e0e0e0;
            transition: all 0.2s ease;
        }

        .reasoning-item:hover {
            background: rgba(153, 69, 255, 0.2);
            border-left-color: #14f195;
        }

        /* Signal Details Styles */
        .signal-details {
            margin: 10px 0;
            padding: 8px;
            background: rgba(0, 0, 0, 0.2);
            border-radius: 6px;
            border: 1px solid rgba(255, 255, 255, 0.1);
        }

        .details-label {
            font-size: 0.85rem;
            font-weight: bold;
            color: #ff6b6b;
            margin-bottom: 6px;
            text-transform: uppercase;
            letter-spacing: 0.5px;
        }

        .details-grid {
            display: grid;
            grid-template-columns: repeat(auto-fit, minmax(120px, 1fr));
            gap: 8px;
        }

        .detail-item {
            display: flex;
            justify-content: space-between;
            align-items: center;
            padding: 4px 6px;
            background: rgba(0, 0, 0, 0.3);
            border-radius: 4px;
            border: 1px solid rgba(255, 255, 255, 0.05);
        }

        .detail-label {
            font-size: 0.75rem;
            color: #888;
            font-weight: 500;
        }

        .detail-value {
            font-size: 0.75rem;
            color: #fff;
            font-weight: bold;
        }

        .detail-value.positive {
            color: #14f195;
        }

        .detail-value.negative {
            color: #ff6b6b;
        }

        .detail-value.neutral {
            color: #9945ff;
        }
        
        @keyframes buySignalGlow {
            0%, 100% {
                box-shadow: 0 0 20px rgba(20, 241, 149, 0.3);
            }
            50% {
                box-shadow: 0 0 30px rgba(20, 241, 149, 0.6);
            }
        }
        
        @keyframes sellSignalGlow {
            0%, 100% {
                box-shadow: 0 0 20px rgba(255, 107, 107, 0.3);
            }
            50% {
                box-shadow: 0 0 30px rgba(255, 107, 107, 0.6);
            }
        }
        
        @keyframes holdSignalGlow {
            0%, 100% {
                box-shadow: 0 0 20px rgba(153, 69, 255, 0.3);
            }
            50% {
                box-shadow: 0 0 30px rgba(153, 69, 255, 0.6);
            }
        }

        .refresh-btn {
            position: fixed;
            bottom: 30px;
            right: 30px;
            background: linear-gradient(145deg, #9945ff, #14f195);
            color: #ffffff;
            border: none;
            border-radius: 50%;
            width: 60px;
            height: 60px;
            font-size: 1.5rem;
            cursor: pointer;
            box-shadow: 0 5px 15px rgba(153, 69, 255, 0.3);
            transition: all 0.3s ease;
        }

        .refresh-btn:hover {
            background: linear-gradient(145deg, #14f195, #9945ff);
            transform: scale(1.1);
            box-shadow: 0 8px 25px rgba(153, 69, 255, 0.5);
        }

        .loading {
            text-align: center;
            padding: 20px;
            color: #9945ff;
        }

        .update-indicator {
            position: fixed;
            top: 20px;
            right: 20px;
            background: linear-gradient(145deg, #14f195, #9945ff);
            color: white;
            padding: 8px 16px;
            border-radius: 20px;
            font-size: 0.8rem;
            font-weight: bold;
            opacity: 0;
            transform: translateY(-20px);
            transition: all 0.3s ease;
            z-index: 1000;
        }

        .update-indicator.show {
            opacity: 1;
            transform: translateY(0);
        }

        .connection-status {
            position: fixed;
            top: 20px;
            left: 20px;
            padding: 8px 16px;
            border-radius: 20px;
            font-size: 0.8rem;
            font-weight: bold;
            z-index: 1000;
        }

        .connection-status.connected {
            background: linear-gradient(145deg, #0d2b1a, #1a2e1a);
            color: #14f195;
            border: 1px solid #14f195;
        }

        .connection-status.disconnected {
            background: linear-gradient(145deg, #2b1a1a, #2e1a1a);
            color: #ff6b6b;
            border: 1px solid #ff6b6b;
        }

        .error {
            background: linear-gradient(145deg, #2b1a1a, #2e1a1a);
            color: #ff6b6b;
            padding: 15px;
            border-radius: 8px;
            margin: 10px 0;
            border: 1px solid #ff6b6b;
        }

        /* Exchange Prices Styles */
        .exchange-grid {
            display: grid;
            grid-template-columns: repeat(auto-fit, minmax(200px, 1fr));
            gap: 15px;
            margin-top: 10px;
        }

        .exchange-card {
            background: linear-gradient(145deg, #0a0a0a, #1a1a1a);
            border: 1px solid #333333;
            border-radius: 10px;
            padding: 15px;
            text-align: center;
            transition: all 0.3s ease;
            position: relative;
            overflow: hidden;
        }

        .exchange-card::before {
            content: '';
            position: absolute;
            top: 0;
            left: 0;
            right: 0;
            height: 2px;
        }

        .exchange-card.connected::before {
            background: linear-gradient(90deg, #14f195, #9945ff);
        }

        .exchange-card.disconnected::before {
            background: linear-gradient(90deg, #ff6b6b, #ff8e53);
        }

        .exchange-card:hover {
            transform: translateY(-2px);
            box-shadow: 0 8px 25px rgba(0,0,0,0.4);
        }

        .exchange-header {
            display: flex;
            justify-content: space-between;
            align-items: center;
            margin-bottom: 10px;
            font-size: 0.9rem;
        }

        .exchange-icon {
            font-size: 1.2rem;
        }

        .exchange-name {
            font-weight: bold;
            color: #9945ff;
        }

        .exchange-pair {
            font-size: 0.8rem;
            color: #888;
        }

        .exchange-price {
            font-size: 1.4rem;
            font-weight: bold;
            color: #14f195;
            margin: 8px 0;
            text-shadow: 0 0 10px rgba(20, 241, 149, 0.3);
        }

        .exchange-card.disconnected .exchange-price {
            color: #ff6b6b;
            text-shadow: 0 0 10px rgba(255, 107, 107, 0.3);
        }

        .exchange-time {
            font-size: 0.75rem;
            color: #666;
        }

        .exchange-card.connected .exchange-time {
            color: #14f195;
        }

        .exchange-card.disconnected .exchange-time {
            color: #ff6b6b;
        }
        
        @keyframes pulse {
            0%, 100% { opacity: 1; }
            50% { opacity: 0.3; }
        }
    </style>
</head>
<body>
    <div class="container">
        <div class="header">
            <h1>TiRADE Dashboard</h1>
            <p>Real-time Solana Trading Bot Monitoring</p>
        </div>

        <div id="loading" class="loading">
            <h3>Loading dashboard data...</h3>
        </div>

        <div id="error" class="error" style="display: none;"></div>

        <div id="dashboard" style="display: none;">
            <!-- Exchange Prices -->
            <div class="card">
                <h3>🏪 Live Exchange Prices</h3>
                <div id="exchange-prices">
                    <div style="text-align: center; padding: 20px; color: #888;">
                        Loading exchange prices...
                    </div>
                </div>
            </div>

            <!-- Current Price & Performance -->
            <div class="card">
                <h3>💰 Current Price & Performance</h3>
                <div id="price-performance"></div>
            </div>

            <!-- Technical Indicators -->
            <div class="card">
                <h3>📊 Technical Indicators</h3>
                <div id="technical-indicators"></div>
            </div>

            <!-- Latest Signals -->
            <div class="card">
                <h3>🎯 Latest Trading Signals</h3>
                <div class="market-sentiment" id="market-sentiment"></div>
                <div id="latest-signals"></div>
            </div>

            <!-- Active Positions -->
            <div class="card">
                <h3>📈 Active Positions</h3>
                <div id="active-positions"></div>
            </div>

            <!-- Recent Trades -->
            <div class="card">
                <h3>💼 Recent Trades</h3>
                <div id="recent-trades"></div>
            </div>

            <!-- Performance Metrics -->
            <div class="card">
                <h3>📈 Performance Metrics</h3>
                <div id="performance-metrics"></div>
            </div>



            <!-- System Status -->
            <div class="card">
                <h3>🔧 System Status</h3>
                <div class="status-grid" id="system-status"></div>
            </div>
        </div>
    </div>

    <div class="connection-status connected" id="connection-status">
        🔗 Real-time Connected
    </div>
    <div class="update-indicator" id="update-indicator">
        🔄 Updating...
    </div>
    <button class="refresh-btn" onclick="loadDashboard()">🔄</button>

    <script>
        let lastSignals = []; // Track previous signals to detect new ones

        async function loadDashboard() {
            try {
                // Show update indicator
                const updateIndicator = document.getElementById('update-indicator');
                updateIndicator.classList.add('show');
                
                // Hide loading on subsequent updates
                if (document.getElementById('dashboard').style.display === 'none') {
                    document.getElementById('loading').style.display = 'block';
                    document.getElementById('dashboard').style.display = 'none';
                }
                document.getElementById('error').style.display = 'none';

                const response = await fetch('/api/dashboard');
                if (!response.ok) throw new Error('Failed to fetch dashboard data');
                
                const data = await response.json();
                
                console.log('Dashboard data received:', {
                    system_status: data.system_status,
                    latest_prices: data.latest_prices?.length || 0,
                    latest_indicators: data.latest_indicators?.length || 0,
                    latest_signals: data.latest_signals?.length || 0,
                    active_positions: data.active_positions?.length || 0,
                    recent_trades: data.recent_trades?.length || 0,
                    price_history: data.price_history?.length || 0
                });
                
                updateSystemStatus(data.system_status);
                updateExchangePrices(data.latest_prices); // Add missing exchange prices update
                updatePricePerformance(data.latest_prices, data.performance, data.price_changes);
                updateTechnicalIndicators(data.latest_indicators);
                updateLatestSignals(data.latest_signals, data.market_sentiment, data.price_changes);
                updateActivePositions(data.active_positions);
                updateRecentTrades(data.recent_trades);
                updatePerformanceMetrics(data.performance);

                document.getElementById('loading').style.display = 'none';
                document.getElementById('dashboard').style.display = 'block';
                
                // Hide update indicator after a short delay
                setTimeout(() => {
                    updateIndicator.classList.remove('show');
                }, 1000);
            } catch (error) {
                console.error('Error loading dashboard:', error);
                document.getElementById('loading').style.display = 'none';
                document.getElementById('error').style.display = 'block';
                document.getElementById('error').textContent = 'Error loading dashboard: ' + error.message;
                
                // Hide update indicator on error
                document.getElementById('update-indicator').classList.remove('show');
            }
        }

        function updateSystemStatus(status) {
            const container = document.getElementById('system-status');
            container.innerHTML = `
                <div class="status-item ${status.database_connected ? 'connected' : 'disconnected'}">
                    <div>🗄️ Database</div>
                    <div>${status.database_connected ? 'Connected' : 'Disconnected'}</div>
                </div>
                <div class="status-item ${status.price_feed_running ? 'connected' : 'disconnected'}">
                    <div>📡 Price Feed</div>
                    <div>${status.price_feed_running ? 'Running' : 'Stopped'}</div>
                </div>
                <div class="status-item ${status.trading_logic_running ? 'connected' : 'disconnected'}">
                    <div>🧠 Trading Logic</div>
                    <div>${status.trading_logic_running ? 'Running' : 'Stopped'}</div>
                </div>
                <div class="status-item ${status.trading_execution_enabled ? 'connected' : 'disconnected'}">
                    <div>⚡ Trading Execution</div>
                    <div>${status.trading_execution_enabled ? 'Enabled' : 'Disabled'}</div>
                </div>
                <div class="status-item ${status.active_position ? 'connected' : 'disconnected'}">
                    <div>📊 Active Position</div>
                    <div>${status.active_position ? 'Yes' : 'No'}</div>
                </div>
                <div class="status-item">
                    <div>🎯 Signals Today</div>
                    <div>${status.total_signals_today}</div>
                </div>
                <div class="status-item">
                    <div>🕒 Last Update</div>
                    <div>${status.last_price_update ? new Date(status.last_price_update).toLocaleTimeString() : 'Never'}</div>
                </div>
            `;
        }

        function updateExchangePrices(prices) {
            const container = document.getElementById('exchange-prices');
            
            if (!prices || prices.length === 0) {
                container.innerHTML = `
                    <div style="text-align: center; padding: 20px; color: #888;">
                        No price data available
                    </div>
                `;
                return;
            }

            // Group prices by source
            const priceMap = {};
            prices.forEach(price => {
                priceMap[price.source.toLowerCase()] = price;
            });

            // Order: Coinbase, Pyth, Jupiter (live API first)
            const sourceOrder = ['coinbase', 'pyth', 'jupiter'];
            const sourceIcons = {
                'pyth': '🔮',
                'jupiter': '🪐',
                'coinbase': '🟢'
            };
            const sourceNames = {
                'pyth': 'Pyth',
                'jupiter': 'Jupiter',
                'coinbase': 'Coinbase'
            };

            let exchangeCards = '';
            sourceOrder.forEach(source => {
                const price = priceMap[source];
                if (price) {
                    const timestamp = new Date(price.timestamp);
                    const isRecent = (Date.now() - timestamp.getTime()) < 5 * 60 * 1000; // 5 minutes
                    const statusClass = isRecent ? 'connected' : 'disconnected';
                    const pair = price.pair.replace('%2F', '/');
                    const isLive = source === 'coinbase'; // Coinbase is live API
                    
                    exchangeCards += `
                        <div class="exchange-card ${statusClass}">
                            <div class="exchange-header">
                                <span class="exchange-icon">${sourceIcons[source]}</span>
                                <span class="exchange-name">${sourceNames[source]}${isLive ? ' 🔴 LIVE' : ''}</span>
                                <span class="exchange-pair">${pair}</span>
                            </div>
                            <div class="exchange-price">$${price.price.toFixed(4)}</div>
                            <div class="exchange-time">${timestamp.toLocaleTimeString()}</div>
                        </div>
                    `;
                } else {
                    exchangeCards += `
                        <div class="exchange-card disconnected">
                            <div class="exchange-header">
                                <span class="exchange-icon">${sourceIcons[source]}</span>
                                <span class="exchange-name">${sourceNames[source]}</span>
                                <span class="exchange-pair">N/A</span>
                            </div>
                            <div class="exchange-price">No Data</div>
                            <div class="exchange-time">Offline</div>
                        </div>
                    `;
                }
            });

            container.innerHTML = `
                <div class="exchange-grid">
                    ${exchangeCards}
                </div>
            `;
        }

        function updatePricePerformance(prices, performance, priceChanges) {
            // Find Pyth price specifically for Current Price & Performance
            const pythPrice = prices.find(price => price.source.toLowerCase() === 'pyth');
            
            // Only use Pyth price for Current Price & Performance section
            // If Pyth is not available, show a message instead of falling back to other sources
            if (!pythPrice) {
                const container = document.getElementById('price-performance');
                container.innerHTML = `
                    <div class="metric">
                        <span>Current Price:</span>
                        <span class="metric-value" style="color: #ff6b6b;">
                            Pyth price unavailable
                            <span style="color: #ff6b6b; font-size: 0.8rem; margin-left: 8px;">⚠️</span>
                        </span>
                    </div>
                    <div class="metric">
                        <span>Status:</span>
                        <span class="metric-value" style="color: #ff6b6b;">Waiting for Pyth data</span>
                    </div>
                `;
                return;
            }
            
            // Use only Pyth price and match the exact formatting from Live Exchange Prices
            const latestPrice = pythPrice;
            const container = document.getElementById('price-performance');
            
            if (latestPrice) {
                // Implement fallback hierarchy for price change display
                let priceChange = null;
                let timeLabel = '';
                
                // Try 24h first, then fall back to 12h, 4h, 1h
                if (priceChanges && priceChanges.change_24h !== undefined && priceChanges.change_24h !== null) {
                    priceChange = priceChanges.change_24h;
                    timeLabel = '24h';
                } else if (priceChanges && priceChanges.change_12h !== undefined && priceChanges.change_12h !== null) {
                    priceChange = priceChanges.change_12h;
                    timeLabel = '12h';
                } else if (priceChanges && priceChanges.change_4h !== undefined && priceChanges.change_4h !== null) {
                    priceChange = priceChanges.change_4h;
                    timeLabel = '4h';
                } else if (priceChanges && priceChanges.change_1h !== undefined && priceChanges.change_1h !== null) {
                    priceChange = priceChanges.change_1h;
                    timeLabel = '1h';
                } else {
                    // Fall back to the old method if no new price changes data
                    priceChange = latestPrice.price_change_percent_24h || 0;
                    timeLabel = '24h';
                }
                
                const changeClass = priceChange > 0 ? 'positive' : priceChange < 0 ? 'negative' : 'neutral';
                const changeSymbol = priceChange > 0 ? '📈' : priceChange < 0 ? '📉' : '➡️';
                
                container.innerHTML = `
                    <div class="metric">
                        <span>${timeLabel} Change:</span>
                        <span class="metric-value ${changeClass}" id="price-change">${changeSymbol} ${priceChange.toFixed(2)}%</span>
                    </div>
                    <div class="metric">
                        <span>Total PnL:</span>
                        <span class="metric-value ${performance.total_pnl >= 0 ? 'positive' : 'negative'}">
                            ${performance.total_pnl >= 0 ? '+' : ''}$${performance.total_pnl.toFixed(2)}
                        </span>
                    </div>
                    <div class="metric">
                        <span>Win Rate:</span>
                        <span class="metric-value">${performance.win_rate.toFixed(1)}%</span>
                    </div>
                `;
            }
        }

        function updateTechnicalIndicators(indicators) {
            const latest = indicators[0];
            const container = document.getElementById('technical-indicators');
            
            if (latest) {
                container.innerHTML = `
                    <div class="metric">
                        <span>RSI (14):</span>
                        <span class="metric-value">${latest.rsi_14 ? latest.rsi_14.toFixed(2) : 'N/A'}</span>
                    </div>
                    <div class="metric">
                        <span>SMA (20):</span>
                        <span class="metric-value">${latest.sma_20 ? '$' + latest.sma_20.toFixed(4) : 'N/A'}</span>
                    </div>
                    <div class="metric">
                        <span>SMA (50):</span>
                        <span class="metric-value">${latest.sma_50 ? '$' + latest.sma_50.toFixed(4) : 'N/A'}</span>
                    </div>
                    <div class="metric">
                        <span>Volatility (24h):</span>
                        <span class="metric-value">${latest.volatility_24h ? (latest.volatility_24h * 100).toFixed(2) + '%' : 'N/A'}</span>
                    </div>
                `;
            }
        }

        function updateLatestSignals(signals, marketSentiment, priceChanges) {
            const container = document.getElementById('latest-signals');
            const sentimentContainer = document.getElementById('market-sentiment');
            
            // Update market sentiment
            if (sentimentContainer) {
                sentimentContainer.innerHTML = `
                    <div class="sentiment-indicator">
                        <span class="sentiment-text">${marketSentiment || '⚖️ Neutral'}</span>
                    </div>
                `;
            }
            
            if (signals.length === 0) {
                container.innerHTML = '<p>No signals generated yet</p>';
                lastSignals = [];
                return;
            }

            // Detect new signals by comparing with previous signals
            const newSignals = signals.filter(signal => {
                return !lastSignals.some(lastSignal => lastSignal.id === signal.id);
            });

            container.innerHTML = signals.slice(0, 5).map(signal => {
                const signalClass = signal.signal_type.toLowerCase();
                const confidenceColor = signal.confidence > 70 ? 'positive' : signal.confidence > 40 ? 'neutral' : 'negative';
                const isNew = newSignals.some(newSignal => newSignal.id === signal.id);
                const newClass = isNew ? 'new-signal' : '';
                
                // Build price change spans only for available intervals
                let priceChangeSpans = '';
                if (priceChanges.change_1h !== undefined && priceChanges.change_1h !== null) {
                    const cls = priceChanges.change_1h >= 0 ? 'positive' : 'negative';
                    const sign = priceChanges.change_1h >= 0 ? '+' : '';
                    priceChangeSpans += `<span class="${cls}">1h: ${sign}${priceChanges.change_1h.toFixed(2)}%</span>`;
                }
                if (priceChanges.change_4h !== undefined && priceChanges.change_4h !== null) {
                    const cls = priceChanges.change_4h >= 0 ? 'positive' : 'negative';
                    const sign = priceChanges.change_4h >= 0 ? '+' : '';
                    priceChangeSpans += `<span class="${cls}">4h: ${sign}${priceChanges.change_4h.toFixed(2)}%</span>`;
                }
                if (priceChanges.change_12h !== undefined && priceChanges.change_12h !== null) {
                    const cls = priceChanges.change_12h >= 0 ? 'positive' : 'negative';
                    const sign = priceChanges.change_12h >= 0 ? '+' : '';
                    priceChangeSpans += `<span class="${cls}">12h: ${sign}${priceChanges.change_12h.toFixed(2)}%</span>`;
                }
                if (priceChanges.change_24h !== undefined && priceChanges.change_24h !== null) {
                    const cls = priceChanges.change_24h >= 0 ? 'positive' : 'negative';
                    const sign = priceChanges.change_24h >= 0 ? '+' : '';
                    priceChangeSpans += `<span class="${cls}">24h: ${sign}${priceChanges.change_24h.toFixed(2)}%</span>`;
                }

                // Parse signal triggers from reasoning
                const triggers = parseSignalTriggers(signal.reasoning);
                
                return `
                    <div class="signal-item signal-${signalClass} ${newClass}" data-signal-id="${signal.id}">
                        <div><strong>${signal.signal_type.toUpperCase()}</strong> - ${(signal.confidence * 100).toFixed(1)}% confidence</div>
                        <div style="display: flex; justify-content: space-between; align-items: center;">
                            <span>Price: $${signal.price.toFixed(4)}</span>
                            <div style="display: flex; gap: 10px; font-size: 0.8rem;">
                                ${priceChangeSpans}
                            </div>
                        </div>
                        <div class="signal-triggers">
                            <div class="triggers-label">Signal Triggers:</div>
                            <div class="triggers-grid">
                                <label class="trigger-checkbox ${triggers.rsiDivergence ? 'active' : ''}">
                                    <input type="checkbox" ${triggers.rsiDivergence ? 'checked' : ''} disabled>
                                    <span>RSI Divergence</span>
                                </label>
                                <label class="trigger-checkbox ${triggers.movingAverage ? 'active' : ''}">
                                    <input type="checkbox" ${triggers.movingAverage ? 'checked' : ''} disabled>
                                    <span>Moving Average</span>
                                </label>
                                <label class="trigger-checkbox ${triggers.volatilityBreakout ? 'active' : ''}">
                                    <input type="checkbox" ${triggers.volatilityBreakout ? 'checked' : ''} disabled>
                                    <span>Volatility Breakout</span>
                                </label>
                                <label class="trigger-checkbox ${triggers.meanReversion ? 'active' : ''}">
                                    <input type="checkbox" ${triggers.meanReversion ? 'checked' : ''} disabled>
                                    <span>Mean Reversion</span>
                                </label>
                                <label class="trigger-checkbox ${triggers.rsiThreshold ? 'active' : ''}">
                                    <input type="checkbox" ${triggers.rsiThreshold ? 'checked' : ''} disabled>
                                    <span>RSI Threshold</span>
                                </label>
                                <label class="trigger-checkbox ${triggers.momentumConfirmation ? 'active' : ''}">
                                    <input type="checkbox" ${triggers.momentumConfirmation ? 'checked' : ''} disabled>
                                    <span>Momentum Confirmation</span>
                                </label>
                                <label class="trigger-checkbox ${triggers.trendFollowing ? 'active' : ''}">
                                    <input type="checkbox" ${triggers.trendFollowing ? 'checked' : ''} disabled>
                                    <span>Trend Following</span>
                                </label>
                                <label class="trigger-checkbox ${triggers.dynamicThresholds ? 'active' : ''}">
                                    <input type="checkbox" ${triggers.dynamicThresholds ? 'checked' : ''} disabled>
                                    <span>Dynamic Thresholds</span>
                                </label>
                                <label class="trigger-checkbox ${triggers.marketRegime ? 'active' : ''}">
                                    <input type="checkbox" ${triggers.marketRegime ? 'checked' : ''} disabled>
                                    <span>Market Regime</span>
                                </label>
                                <label class="trigger-checkbox ${triggers.supportResistance ? 'active' : ''}">
                                    <input type="checkbox" ${triggers.supportResistance ? 'checked' : ''} disabled>
                                    <span>Support/Resistance</span>
                                </label>
                            </div>
                        </div>
                        <div class="signal-reasoning">
                            <div class="reasoning-label">Detailed Analysis:</div>
                            <div class="reasoning-text">
                                ${Array.isArray(signal.reasoning) ? 
                                    signal.reasoning.map(reason => 
                                        `<div class="reasoning-item">• ${reason}</div>`
                                    ).join('') : 
                                    `<div class="reasoning-item">• ${signal.reasoning}</div>`
                                }
                            </div>
                        </div>
                        <div class="signal-details">
                            <div class="details-label">Signal Details:</div>
                            <div class="details-grid">
                                <div class="detail-item">
                                    <span class="detail-label">Take Profit:</span>
                                    <span class="detail-value">${signal.take_profit ? (signal.take_profit * 100).toFixed(2) + '%' : 'N/A'}</span>
                                </div>
                                <div class="detail-item">
                                    <span class="detail-label">Stop Loss:</span>
                                    <span class="detail-value">${signal.stop_loss ? (signal.stop_loss * 100).toFixed(2) + '%' : 'N/A'}</span>
                                </div>
                                <div class="detail-item">
                                    <span class="detail-label">Executed:</span>
                                    <span class="detail-value ${signal.executed ? 'positive' : 'neutral'}">${signal.executed ? '✅ Yes' : '⏳ Pending'}</span>
                                </div>
                            </div>
                        </div>
                        <div><small>${new Date(signal.timestamp).toLocaleString()}</small></div>
                    </div>
                `;
            }).join('');

            // Add notification sound for new signals (if supported)
            if (newSignals.length > 0) {
                console.log(`🎯 New signal detected: ${newSignals[0].signal_type.toUpperCase()}`);
                
                // Remove new-signal class after animation completes
                setTimeout(() => {
                    newSignals.forEach(signal => {
                        const element = document.querySelector(`[data-signal-id="${signal.id}"]`);
                        if (element) {
                            element.classList.remove('new-signal');
                        }
                    });
                }, 3000);
            }

            // Update last signals
            lastSignals = signals.slice(0, 5);
        }

        function parseSignalTriggers(reasoning) {
            const triggers = {
                rsiDivergence: false,
                movingAverage: false,
                volatilityBreakout: false,
                meanReversion: false,
                rsiThreshold: false,
                momentumConfirmation: false,
                trendFollowing: false,
                dynamicThresholds: false,
                marketRegime: false,
                supportResistance: false
            };

            // Convert reasoning to string if it's an array
            const reasoningText = Array.isArray(reasoning) ? reasoning.join(' ') : reasoning;

            // Enhanced trigger detection for new trading logic
            if (reasoningText.includes('RSI divergence') || 
                (reasoningText.includes('Fast RSI') && reasoningText.includes('Slow RSI')) ||
                reasoningText.includes('RSI divergence: Fast RSI') ||
                reasoningText.includes('RSI divergence detected')) {
                triggers.rsiDivergence = true;
            }
            if (reasoningText.includes('uptrend') || 
                reasoningText.includes('downtrend') || 
                reasoningText.includes('SMA ratio') ||
                reasoningText.includes('Moving average crossover') ||
                reasoningText.includes('SMA') ||
                reasoningText.includes('trend strength')) {
                triggers.movingAverage = true;
            }
            if (reasoningText.includes('Volatility breakout') || 
                (reasoningText.includes('volatility') && reasoningText.includes('momentum')) ||
                reasoningText.includes('volatility breakout') ||
                reasoningText.includes('volatility')) {
                triggers.volatilityBreakout = true;
            }
            if (reasoningText.includes('Mean reversion') || 
                reasoningText.includes('Extreme oversold') || 
                reasoningText.includes('Extreme overbought') ||
                reasoningText.includes('Mean reversion: Extreme oversold') ||
                reasoningText.includes('oversold') ||
                reasoningText.includes('overbought')) {
                triggers.meanReversion = true;
            }
            if (reasoningText.includes('RSI overbought') || 
                reasoningText.includes('RSI oversold') ||
                reasoningText.includes('RSI oversold: RSI') ||
                reasoningText.includes('RSI overbought: RSI') ||
                reasoningText.includes('RSI threshold')) {
                triggers.rsiThreshold = true;
            }
            if (reasoningText.includes('momentum') || 
                reasoningText.includes('momentum confirmation') ||
                reasoningText.includes('price momentum')) {
                triggers.momentumConfirmation = true;
            }
            if (reasoningText.includes('trend following') || 
                reasoningText.includes('trend strength') ||
                reasoningText.includes('market regime')) {
                triggers.trendFollowing = true;
            }
            if (reasoningText.includes('dynamic threshold') || 
                reasoningText.includes('dynamic take profit') ||
                reasoningText.includes('dynamic stop loss')) {
                triggers.dynamicThresholds = true;
            }
            if (reasoningText.includes('market regime') || 
                reasoningText.includes('trending') ||
                reasoningText.includes('consolidating') ||
                reasoningText.includes('volatile')) {
                triggers.marketRegime = true;
            }
            if (reasoningText.includes('support') || 
                reasoningText.includes('resistance') ||
                reasoningText.includes('support level') ||
                reasoningText.includes('resistance level')) {
                triggers.supportResistance = true;
            }
            if (reasoningText.includes('Momentum confirmation') || 
                reasoningText.includes('price increase') || 
                reasoningText.includes('price decrease') ||
                reasoningText.includes('momentum confirmation')) {
                triggers.momentumConfirmation = true;
            }
            if (reasoningText.includes('Trend following') || 
                reasoningText.includes('above SMA') || 
                reasoningText.includes('below SMA') ||
                reasoningText.includes('Enhanced trend following') ||
                reasoningText.includes('below SMA') && reasoningText.includes('RSI') && reasoningText.includes('bearish range')) {
                triggers.trendFollowing = true;
            }

            return triggers;
        }
        
        function updateActivePositions(positions) {
            const container = document.getElementById('active-positions');
            
            if (positions.length === 0) {
                container.innerHTML = '<p>No active positions</p>';
                return;
            }

            container.innerHTML = positions.map(position => {
                const pnlClass = position.pnl >= 0 ? 'positive' : 'negative';
                const pnlSymbol = position.pnl >= 0 ? '+' : '';
                
                return `
                    <div class="metric">
                        <span>${position.position_type} ${position.pair}</span>
                        <span class="metric-value ${pnlClass}">
                            ${pnlSymbol}$${position.pnl.toFixed(2)} (${pnlSymbol}${position.pnl_percent.toFixed(2)}%)
                        </span>
                    </div>
                `;
            }).join('');
        }

        function updateRecentTrades(trades) {
            const container = document.getElementById('recent-trades');
            
            if (trades.length === 0) {
                container.innerHTML = '<p>No recent trades</p>';
                return;
            }

            container.innerHTML = trades.slice(0, 5).map(trade => {
                return `
                    <div class="metric">
                        <span>${trade.trade_type} ${trade.pair}</span>
                        <span class="metric-value">$${trade.total_value.toFixed(2)}</span>
                    </div>
                `;
            }).join('');
        }

        function updatePerformanceMetrics(performance) {
            const container = document.getElementById('performance-metrics');
            
            container.innerHTML = `
                <div class="metric">
                    <span>Total Trades:</span>
                    <span class="metric-value">${performance.total_trades}</span>
                </div>
                <div class="metric">
                    <span>Winning Trades:</span>
                    <span class="metric-value positive">${performance.winning_trades}</span>
                </div>
                <div class="metric">
                    <span>Losing Trades:</span>
                    <span class="metric-value negative">${performance.losing_trades}</span>
                </div>
                <div class="metric">
                    <span>Win Rate:</span>
                    <span class="metric-value">${performance.win_rate.toFixed(1)}%</span>
                </div>
                <div class="metric">
                    <span>Total Volume:</span>
                    <span class="metric-value">$${performance.total_volume.toFixed(2)}</span>
                </div>
                <div class="metric">
                    <span>Sharpe Ratio:</span>
                    <span class="metric-value">${performance.sharpe_ratio.toFixed(2)}</span>
                </div>
            `;
        }



        // Real-time updates using Server-Sent Events
        const dashboardEventSource = new EventSource('/api/dashboard/stream');
        const priceEventSource = new EventSource('/api/price/stream');
        
        // Dashboard updates (every 30 seconds)
        dashboardEventSource.onmessage = function(event) {
            try {
                const data = JSON.parse(event.data);
                if (data.type === 'dashboard_update') {
                    console.log('Received dashboard update:', data.message);
                    loadDashboard();
                }
            } catch (error) {
                console.error('Error parsing dashboard SSE data:', error);
            }
        };
        
        // Price updates (every second)
        priceEventSource.onmessage = function(event) {
            try {
                const data = JSON.parse(event.data);
                if (data.type === 'price_update') {
                    console.log('Received price update:', data.message);
                    updatePrices();
                }
            } catch (error) {
                console.error('Error parsing price SSE data:', error);
            }
        };
        
        dashboardEventSource.onopen = function(event) {
            console.log('Dashboard SSE connection established');
            document.getElementById('connection-status').className = 'connection-status connected';
            document.getElementById('connection-status').textContent = '🔗 Real-time Connected';
        };
        
        priceEventSource.onopen = function(event) {
            console.log('Price SSE connection established');
        };
        
        dashboardEventSource.onerror = function(error) {
            console.error('Dashboard SSE connection error:', error);
            document.getElementById('connection-status').className = 'connection-status disconnected';
            document.getElementById('connection-status').textContent = '❌ Connection Lost';
            
            // Fallback to polling if SSE fails
            setTimeout(() => {
                console.log('Falling back to polling...');
                setInterval(loadDashboard, 5000); // Refresh every 5 seconds instead of 30
            }, 5000);
        };
        
        priceEventSource.onerror = function(error) {
            console.error('Price SSE connection error:', error);
            // Fallback to polling for prices
            setTimeout(() => {
                console.log('Falling back to price polling...');
                setInterval(updatePrices, 1000);
            }, 2000);
        };
        
        // Function to update only prices (faster than full dashboard reload)
        async function updatePrices() {
            try {
                // Fetch latest price from the dashboard API (which gets it from database)
                const response = await fetch('/api/dashboard');
                const data = await response.json();
                
                if (data.latest_prices && data.latest_prices.length > 0) {
                    const solPrice = data.latest_prices.find(p => p.pair === 'SOL/USDC');
                    if (solPrice) {
                        // Update current price display
                        const priceElement = document.getElementById('current-price');
                        if (priceElement) {
                            priceElement.innerHTML = `$${solPrice.price.toFixed(4)}<span style="color: #14f195; font-size: 0.8rem; margin-left: 8px; animation: pulse 1s ease-in-out infinite;">●</span>`;
                        }
                        
                        // Update price change if available
                        if (data.latest_indicators && data.latest_indicators.length > 0) {
                            const indicator = data.latest_indicators[0];
                            if (indicator.price_change_percent_24h) {
                                const changeElement = document.getElementById('price-change');
                                if (changeElement) {
                                    const change = indicator.price_change_percent_24h;
                                    const changeSymbol = change >= 0 ? '📈' : '📉';
                                    changeElement.textContent = `${changeSymbol} ${change >= 0 ? '+' : ''}${change.toFixed(2)}%`;
                                    changeElement.className = change >= 0 ? 'positive' : 'negative';
                                }
                            }
                        }
                        
                        // Don't update chart during price updates to avoid conflicts
                        // Chart will be updated during full dashboard refresh
                    }
                }
                
            } catch (error) {
                console.error('Error updating prices:', error);
            }
        }

        // Track last signal timestamp for new signal detection
        let lastSignalTimestamp = null;
        
        // Function to update trading signals in real-time
        async function updateSignals() {
            try {
                const response = await fetch('/api/signals/realtime');
                const signals = await response.json();
                
                const signalsContainer = document.getElementById('signals-container');
                if (signalsContainer) {
                    if (signals.length === 0) {
                        signalsContainer.innerHTML = '<p>No signals generated yet</p>';
                    } else {
                        // Check for new signals
                        const latestSignal = signals[0];
                        const isNewSignal = lastSignalTimestamp === null || 
                                          new Date(latestSignal.timestamp) > new Date(lastSignalTimestamp);
                        
                        if (isNewSignal && lastSignalTimestamp !== null) {
                            // Flash notification for new signal
                            const notification = document.createElement('div');
                            notification.style.cssText = `
                                position: fixed; top: 20px; right: 20px; 
                                background: linear-gradient(145deg, #0a0a0a, #1a1a1a); 
                                border: 2px solid #14f195; border-radius: 10px; 
                                padding: 15px; z-index: 1000; color: #14f195;
                                font-weight: bold; animation: slideIn 0.5s ease-out;
                            `;
                            notification.textContent = `🆕 New ${latestSignal.signal_type.toUpperCase()} signal detected!`;
                            document.body.appendChild(notification);
                            
                            // Remove notification after 3 seconds
                            setTimeout(() => {
                                notification.remove();
                            }, 3000);
                            

                        }
                        
                        // Update last signal timestamp
                        if (signals.length > 0) {
                            lastSignalTimestamp = signals[0].timestamp;
                        }
                        
                        signalsContainer.innerHTML = signals.slice(0, 5).map((signal, index) => {
                            const signalType = signal.signal_type.toLowerCase();
                            const signalIcon = signalType === 'buy' ? '🟢' : signalType === 'sell' ? '🔴' : '🟡';
                            const signalColor = signalType === 'buy' ? '#14f195' : signalType === 'sell' ? '#ff6b6b' : '#f7931a';
                            const confidencePercent = (signal.confidence * 100).toFixed(1);
                            
                            // Highlight newest signal
                            const isNewest = index === 0 && isNewSignal;
                            const highlightStyle = isNewest ? 'box-shadow: 0 0 20px rgba(20, 241, 149, 0.5); animation: pulse 2s ease-in-out;' : '';
                            
                            return `
                                <div style="background: linear-gradient(145deg, #0a0a0a, #1a1a1a); border: 1px solid ${signalColor}; border-radius: 10px; padding: 15px; margin-bottom: 10px; ${highlightStyle}">
                                    <div style="display: flex; justify-content: space-between; align-items: center; margin-bottom: 8px;">
                                        <span style="font-size: 1.2rem;">${signalIcon} ${signal.signal_type.toUpperCase()}</span>
                                        <span style="color: ${signalColor}; font-weight: bold;">${confidencePercent}%</span>
                                    </div>
                                    <div style="color: #14f195; font-size: 1.1rem; font-weight: bold; margin-bottom: 5px;">$${signal.price.toFixed(4)}</div>
                                    <div style="color: #888; font-size: 0.9rem; margin-bottom: 8px;">${signal.reasoning}</div>
                                    <div style="color: #666; font-size: 0.8rem;">${new Date(signal.timestamp).toLocaleString()}</div>
                                </div>
                            `;
                        }).join('');
                    }
                }
            } catch (error) {
                console.error('Error updating signals:', error);
            }
        }
        
        // Function to fetch prices from multiple exchanges
        async function fetchExchangePrices() {
            try {
                const [binanceResponse, coinbaseResponse, pythResponse, jupiterResponse] = await Promise.allSettled([
                    fetch('https://api.binance.com/api/v3/ticker/price?symbol=SOLUSDT'),
                    fetch('https://api.coinbase.com/v2/prices/SOL-USD/spot'),
                    fetch('/api/dashboard'), // Pyth price from your existing dashboard API
                    fetch('http://localhost:8080/prices/SOL%2FUSDC/latest?source=jupiter') // Jupiter price directly from database
                ]);

                const prices = {};

                // Binance price
                if (binanceResponse.status === 'fulfilled' && binanceResponse.value.ok) {
                    try {
                        const binanceData = await binanceResponse.value.json();
                        if (binanceData.price) {
                            prices.binance = parseFloat(binanceData.price);
                        }
                    } catch (e) {
                        console.log('Binance data parse error:', e);
                    }
                }

                // Coinbase price
                if (coinbaseResponse.status === 'fulfilled' && coinbaseResponse.value.ok) {
                    try {
                        const coinbaseData = await coinbaseResponse.value.json();
                        if (coinbaseData.data && coinbaseData.data.amount) {
                            prices.coinbase = parseFloat(coinbaseData.data.amount);
                        }
                    } catch (e) {
                        console.log('Coinbase data parse error:', e);
                    }
                }

                // Pyth price from your price feed
                if (pythResponse.status === 'fulfilled' && pythResponse.value.ok) {
                    try {
                        const pythData = await pythResponse.value.json();
                        // Pyth price is in the latest_indicators array
                        if (pythData.latest_indicators && pythData.latest_indicators.length > 0) {
                            const latestIndicator = pythData.latest_indicators[0];
                            if (latestIndicator.current_price) {
                                prices.pyth = parseFloat(latestIndicator.current_price);
                            }
                        }
                    } catch (e) {
                        console.log('Pyth data parse error:', e);
                    }
                }

                // Jupiter price from your price feed
                if (jupiterResponse.status === 'fulfilled' && jupiterResponse.value.ok) {
                    try {
                        const jupiterData = await jupiterResponse.value.json();
                        // Jupiter price is directly in the data field
                        if (jupiterData.data && jupiterData.data.price) {
                            prices.jupiter = parseFloat(jupiterData.data.price);
                        }
                    } catch (e) {
                        console.log('Jupiter data parse error:', e);
                    }
                }

                return prices;
            } catch (error) {
                console.error('Error fetching exchange prices:', error);
                return {};
            }
        }

        // Function to update exchange prices display
        async function updateExchangePrices() {
            const prices = await fetchExchangePrices();
            
            // Update the dedicated exchange prices section
            const exchangeElement = document.getElementById('exchange-prices');
            if (exchangeElement) {
                if (Object.keys(prices).length === 0) {
                    exchangeElement.innerHTML = `
                        <div style="text-align: center; padding: 20px; color: #ff6b6b;">
                            ⚠️ Unable to fetch exchange prices
                        </div>
                    `;
                    return;
                }
                
                let priceHtml = '<div style="display: grid; grid-template-columns: repeat(auto-fit, minmax(200px, 1fr)); gap: 20px;">';
                
                // Binance - only show if price is available
                if (prices.binance) {
                    priceHtml += `
                        <div style="background: linear-gradient(145deg, #0a0a0a, #1a1a1a); border: 1px solid #f7931a; border-radius: 10px; padding: 15px; text-align: center;">
                            <div style="color: #f7931a; font-size: 1.1rem; font-weight: bold; margin-bottom: 10px;">Binance</div>
                            <div style="color: #14f195; font-size: 1.5rem; font-weight: bold;">$${prices.binance.toFixed(4)}</div>
                            <div style="color: #888; font-size: 0.8rem; margin-top: 5px;">SOL/USDT</div>
                        </div>
                    `;
                }
                // Coinbase - only show if price is available
                if (prices.coinbase) {
                    priceHtml += `
                        <div style="background: linear-gradient(145deg, #0a0a0a, #1a1a1a); border: 1px solid #0052ff; border-radius: 10px; padding: 15px; text-align: center;">
                            <div style="color: #0052ff; font-size: 1.1rem; font-weight: bold; margin-bottom: 10px;">Coinbase</div>
                            <div style="color: #14f195; font-size: 1.5rem; font-weight: bold;">$${prices.coinbase.toFixed(4)}</div>
                            <div style="color: #888; font-size: 0.8rem; margin-top: 5px;">SOL/USD</div>
                        </div>
                    `;
                }
                // Pyth - only show if price is available
                if (prices.pyth) {
                    priceHtml += `
                        <div style="background: linear-gradient(145deg, #0a0a0a, #1a1a1a); border: 1px solid #9945ff; border-radius: 10px; padding: 15px; text-align: center;">
                            <div style="color: #9945ff; font-size: 1.1rem; font-weight: bold; margin-bottom: 10px;">PYTH</div>
                            <div style="color: #14f195; font-size: 1.5rem; font-weight: bold;">$${prices.pyth.toFixed(4)}</div>
                            <div style="color: #888; font-size: 0.8rem; margin-top: 5px;">SOL/USD</div>
                        </div>
                    `;
                }
                // JUP - only show if price is available
                if (prices.jupiter) {
                    priceHtml += `
                        <div style="background: linear-gradient(145deg, #0a0a0a, #1a1a1a); border: 1px solid #ff6b35; border-radius: 10px; padding: 15px; text-align: center;">
                            <div style="color: #ff6b35; font-size: 1.1rem; font-weight: bold; margin-bottom: 10px;">JUP</div>
                            <div style="color: #14f195; font-size: 1.5rem; font-weight: bold;">$${prices.jupiter.toFixed(4)}</div>
                            <div style="color: #888; font-size: 0.8rem; margin-top: 5px;">SOL/USDC</div>
                        </div>
                    `;
                }
                
                priceHtml += '</div>';
                

                
                exchangeElement.innerHTML = priceHtml;
            }
            
            // Also update the main price display with the first available price
            const priceElement = document.getElementById('current-price');
            if (priceElement && Object.keys(prices).length > 0) {
                const firstPrice = prices.binance || prices.coinbase || prices.pyth || prices.jupiter;
                if (firstPrice) {
                    priceElement.innerHTML = `$${firstPrice.toFixed(4)}<span style="color: #14f195; font-size: 0.8rem; margin-left: 8px; animation: pulse 1s ease-in-out infinite;">●</span>`;
                }
            }
        }

        // Update exchange prices every 1 second
        setInterval(updateExchangePrices, 1000);
        
        // Update signals every 10 seconds (trading logic runs every 30 seconds)
        setInterval(updateSignals, 10000);
        
        // Initial fetch
        updateExchangePrices();
        updateSignals();
        
        // Initial load
        loadDashboard();
    </script>
</body>
</html>
    "#;
    
    Ok(HttpResponse::Ok()
        .content_type("text/html")
        .body(html))
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    // Load environment variables from .env file
    dotenv::dotenv().ok();
    
    let database_url = std::env::var("DATABASE_URL").unwrap_or_else(|_| "http://localhost:8080".to_string());
    let bind_address = std::env::var("DASHBOARD_BIND").unwrap_or_else(|_| "0.0.0.0".to_string());
    let bind_port = std::env::var("DASHBOARD_PORT").unwrap_or_else(|_| "3000".to_string());
    let bind_addr = format!("{}:{}", bind_address, bind_port);
    
    println!("🚀 Starting Tirade Dashboard on http://{}", bind_addr);
    println!("📊 Database URL: {}", database_url);
    println!("🌐 External Access: http://YOUR_VM_PUBLIC_IP:{}", bind_port);

    let app_state = web::Data::new(AppState {
        database_url,
        cache: Arc::new(Mutex::new(HashMap::new())),
        client: reqwest::Client::new(),
    });

    HttpServer::new(move || {
        App::new()
            .app_data(app_state.clone())
            .route("/", web::get().to(index))
            .route("/api/dashboard", web::get().to(get_dashboard_data))
            .route("/api/signals/realtime", web::get().to(get_realtime_signals))
            .route("/api/dashboard/stream", web::get().to(dashboard_stream))
            .route("/api/price/stream", web::get().to(price_stream))
            .service(Files::new("/static", "./static").show_files_listing())
    })
    .bind(&bind_addr)?
    .run()
    .await
} 