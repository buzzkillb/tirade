use actix_web::{web, App, HttpServer, HttpResponse, Result, Error};
use actix_files::Files;
use actix_web::web::Bytes;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use chrono::{DateTime, Utc};
use reqwest;
use futures::stream::{Stream, StreamExt};
use std::pin::Pin;
use std::task::{Context, Poll};

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
    pub active_positions: i64,
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

struct AppState {
    database_url: String,
    cache: Arc<Mutex<HashMap<String, DashboardData>>>,
}

async fn get_dashboard_data(state: web::Data<AppState>) -> Result<HttpResponse> {
    let client = reqwest::Client::new();
    let mut dashboard_data = DashboardData {
        system_status: SystemStatus {
            database_connected: false,
            price_feed_running: false,
            trading_logic_running: false,
            trading_execution_enabled: false,
            last_price_update: None,
            last_signal_generated: None,
            active_positions: 0,
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

    // Fetch latest prices - try Pyth first (SOL/USD), then fall back to any SOL/USDC source
    let mut pyth_price: Option<PriceData> = None;
    
    // Try to get Pyth price from SOL/USD (what Pyth actually stores)
    if let Ok(response) = client
        .get(&format!("{}/prices/SOL%2FUSD/latest?source=pyth", state.database_url))
        .send()
        .await
    {
        if let Ok(api_response) = response.json::<serde_json::Value>().await {
            if let Some(price_data) = api_response["data"].as_object() {
                if let Ok(price) = serde_json::from_value::<PriceData>(serde_json::Value::Object(price_data.clone())) {
                    pyth_price = Some(price);
                }
            }
        }
    }
    
    // If we have Pyth data, use it; otherwise fall back to any SOL/USDC source
    if let Some(price) = pyth_price {
        dashboard_data.latest_prices = vec![price.clone()];
        dashboard_data.system_status.last_price_update = Some(price.timestamp);
        // Check if price is recent (within last 5 minutes)
        let five_minutes_ago = chrono::Utc::now() - chrono::Duration::minutes(5);
        dashboard_data.system_status.price_feed_running = price.timestamp > five_minutes_ago;
    } else {
        // Fall back to any SOL/USDC source (Jupiter, etc.)
        if let Ok(response) = client
            .get(&format!("{}/prices/SOL%2FUSDC/latest", state.database_url))
            .send()
            .await
        {
            if let Ok(api_response) = response.json::<serde_json::Value>().await {
                if let Some(price_data) = api_response["data"].as_object() {
                    if let Ok(price) = serde_json::from_value::<PriceData>(serde_json::Value::Object(price_data.clone())) {
                        dashboard_data.latest_prices = vec![price.clone()];
                        dashboard_data.system_status.last_price_update = Some(price.timestamp);
                        // Check if price is recent (within last 5 minutes)
                        let five_minutes_ago = chrono::Utc::now() - chrono::Duration::minutes(5);
                        dashboard_data.system_status.price_feed_running = price.timestamp > five_minutes_ago;
                    }
                }
            }
        }
    }

    // Fetch latest indicators
    if let Ok(response) = client
        .get(&format!("{}/indicators/SOL%2FUSDC", state.database_url))
        .send()
        .await
    {
        if let Ok(api_response) = response.json::<serde_json::Value>().await {
            if let Some(indicator_data) = api_response["data"].as_object() {
                // Create a TechnicalIndicator with the available data
                let mut indicator = TechnicalIndicator {
                    id: "latest".to_string(),
                    pair: indicator_data["pair"].as_str().unwrap_or("SOL/USDC").to_string(),
                    timestamp: chrono::Utc::now(),
                    sma_20: indicator_data["sma_20"].as_f64(),
                    sma_50: indicator_data["sma_50"].as_f64(),
                    sma_200: indicator_data["sma_200"].as_f64(),
                    rsi_14: indicator_data["rsi_14"].as_f64(),
                    price_change_24h: indicator_data["price_change_24h"].as_f64(),
                    price_change_percent_24h: indicator_data["price_change_percent_24h"].as_f64(),
                    volatility_24h: indicator_data["volatility_24h"].as_f64(),
                    current_price: indicator_data["current_price"].as_f64().unwrap_or(0.0),
                    created_at: chrono::Utc::now(),
                };
                
                // Parse timestamp if available
                if let Some(timestamp_str) = indicator_data["timestamp"].as_str() {
                    if let Ok(timestamp) = chrono::DateTime::parse_from_rfc3339(timestamp_str) {
                        indicator.timestamp = timestamp.with_timezone(&chrono::Utc);
                    }
                }
                
                dashboard_data.latest_indicators = vec![indicator];
            }
        }
    }

    // Fetch latest signals
    if let Ok(response) = client
        .get(&format!("{}/signals/SOL%2FUSDC", state.database_url))
        .send()
        .await
    {
        if let Ok(api_response) = response.json::<serde_json::Value>().await {
            if let Some(signals_array) = api_response["data"].as_array() {
                if let Ok(signals) = serde_json::from_value::<Vec<TradingSignal>>(serde_json::Value::Array(signals_array.clone())) {
                    // Calculate market sentiment based on signals from last 4 hours
                    let four_hours_ago = chrono::Utc::now() - chrono::Duration::hours(4);
                    let recent_signals = signals.iter()
                        .filter(|s| s.timestamp > four_hours_ago)
                        .collect::<Vec<_>>();
                    let bullish_count = recent_signals.iter().filter(|s| s.signal_type.to_lowercase() == "buy").count();
                    let bearish_count = recent_signals.iter().filter(|s| s.signal_type.to_lowercase() == "sell").count();
                    
                    if bullish_count > bearish_count {
                        dashboard_data.market_sentiment = format!("🐂 Bullish ({} bullish, {} bearish signals)", bullish_count, bearish_count);
                    } else if bearish_count > bullish_count {
                        dashboard_data.market_sentiment = format!("🐻 Bearish ({} bullish, {} bearish signals)", bullish_count, bearish_count);
                    } else {
                        dashboard_data.market_sentiment = format!("⚖️ Neutral ({} bullish, {} bearish signals)", bullish_count, bearish_count);
                    }
                    
                    dashboard_data.latest_signals = signals;
                    if let Some(latest_signal) = dashboard_data.latest_signals.first() {
                        dashboard_data.system_status.last_signal_generated = Some(latest_signal.timestamp);
                        // Check if signal is recent (within last 10 minutes)
                        let ten_minutes_ago = chrono::Utc::now() - chrono::Duration::minutes(10);
                        dashboard_data.system_status.trading_logic_running = latest_signal.timestamp > ten_minutes_ago;
                    }
                }
            }
        }
    }

    // Fetch active positions
    if let Ok(response) = client
        .get(&format!("{}/positions/active", state.database_url))
        .send()
        .await
    {
        if let Ok(api_response) = response.json::<serde_json::Value>().await {
            if let Some(positions_array) = api_response["data"].as_array() {
                if let Ok(positions) = serde_json::from_value::<Vec<Position>>(serde_json::Value::Array(positions_array.clone())) {
                    dashboard_data.active_positions = positions;
                    dashboard_data.system_status.active_positions = dashboard_data.active_positions.len() as i64;
                }
            }
        }
    }

    // Fetch recent trades
    if let Ok(response) = client
        .get(&format!("{}/trades/recent", state.database_url))
        .send()
        .await
    {
        if let Ok(api_response) = response.json::<serde_json::Value>().await {
            if let Some(trades_array) = api_response["data"].as_array() {
                if let Ok(trades) = serde_json::from_value::<Vec<Trade>>(serde_json::Value::Array(trades_array.clone())) {
                    dashboard_data.recent_trades = trades;
                }
            }
        }
    }

    // Fetch performance metrics
    if let Ok(response) = client
        .get(&format!("{}/performance/metrics", state.database_url))
        .send()
        .await
    {
        if let Ok(performance) = response.json::<PerformanceMetrics>().await {
            dashboard_data.performance = performance;
        }
    }

    // Fetch price history for chart (get 48 hours to ensure we have enough data for all time intervals)
    if let Ok(response) = client
        .get(&format!("{}/prices/SOL%2FUSDC/history?hours=48", state.database_url))
        .send()
        .await
    {
        if let Ok(api_response) = response.json::<serde_json::Value>().await {
            if let Some(price_history_array) = api_response["data"].as_array() {
                if let Ok(price_history) = serde_json::from_value::<Vec<PriceData>>(serde_json::Value::Array(price_history_array.clone())) {
                    dashboard_data.price_history = price_history.clone();
                    
                    // Calculate price changes if we have enough data
                    if price_history.len() > 0 {
                        let current_price = price_history[0].price;
                        let now = chrono::Utc::now();
                        
                        // Sort price history by timestamp (newest first) to ensure proper order
                        let mut sorted_history = price_history.clone();
                        sorted_history.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
                        
                        // Find prices at different time intervals
                        let target_1h = now - chrono::Duration::hours(1);
                        let target_4h = now - chrono::Duration::hours(4);
                        let target_12h = now - chrono::Duration::hours(12);
                        let target_24h = now - chrono::Duration::hours(24);
                        
                        // Find the closest price to each target time
                        let price_1h = sorted_history.iter()
                            .filter(|p| p.timestamp <= target_1h)
                            .next()
                            .map(|p| p.price);
                        let price_4h = sorted_history.iter()
                            .filter(|p| p.timestamp <= target_4h)
                            .next()
                            .map(|p| p.price);
                        let price_12h = sorted_history.iter()
                            .filter(|p| p.timestamp <= target_12h)
                            .next()
                            .map(|p| p.price);
                        let price_24h = sorted_history.iter()
                            .filter(|p| p.timestamp <= target_24h)
                            .next()
                            .map(|p| p.price);
                        
                        dashboard_data.price_changes = PriceChanges {
                            change_1h: price_1h.and_then(|p| if p > 0.0 && p != current_price { Some(((current_price - p) / p) * 100.0) } else { None }),
                            change_4h: price_4h.and_then(|p| if p > 0.0 && p != current_price { Some(((current_price - p) / p) * 100.0) } else { None }),
                            change_12h: price_12h.and_then(|p| if p > 0.0 && p != current_price { Some(((current_price - p) / p) * 100.0) } else { None }),
                            change_24h: price_24h.and_then(|p| if p > 0.0 && p != current_price { Some(((current_price - p) / p) * 100.0) } else { None }),
                        };
                    }
                }
            }
        }
    }

    // Check database connection
    if let Ok(response) = client.get(&format!("{}/health", state.database_url)).send().await {
        dashboard_data.system_status.database_connected = response.status().is_success();
    }

    // Check trading execution status from environment variable
    let trading_execution_enabled = std::env::var("ENABLE_TRADING_EXECUTION")
        .unwrap_or_else(|_| "false".to_string())
        .parse::<bool>()
        .unwrap_or(false);
    dashboard_data.system_status.trading_execution_enabled = trading_execution_enabled;

    // Count signals today
    if let Ok(response) = client
        .get(&format!("{}/signals/SOL%2FUSDC/count?hours=24", state.database_url))
        .send()
        .await
    {
        if let Ok(count_response) = response.json::<serde_json::Value>().await {
            if let Some(count) = count_response["data"]["count"].as_i64() {
                dashboard_data.system_status.total_signals_today = count;
            }
        }
    }

    Ok(HttpResponse::Ok().json(dashboard_data))
}

struct DashboardStream {
    state: web::Data<AppState>,
    interval: tokio::time::Interval,
}

struct PriceStream {
    state: web::Data<AppState>,
    interval: tokio::time::Interval,
}

impl Stream for DashboardStream {
    type Item = Result<Bytes, Error>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        if self.interval.poll_tick(cx).is_ready() {
            // This is a simplified version - in a real implementation, you'd fetch fresh data
            let data = serde_json::json!({
                "type": "dashboard_update",
                "timestamp": chrono::Utc::now().to_rfc3339(),
                "message": "Dashboard data updated"
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
            let state = self.state.clone();
            
            // For now, we'll send a simple update message
            // In a production system, you'd want to implement a proper async stream
            // that can fetch and send real-time price data
            let data = serde_json::json!({
                "type": "price_update",
                "timestamp": chrono::Utc::now().to_rfc3339(),
                "message": "Price data updated - fetch latest from /api/prices/latest"
            });
            
            let json_str = data.to_string();
            let bytes = format!("data: {}\n\n", json_str);
            
            Poll::Ready(Some(Ok(Bytes::from(bytes))))
        } else {
            Poll::Pending
        }
    }
}

async fn dashboard_stream(state: web::Data<AppState>) -> Result<HttpResponse> {
    let stream = DashboardStream {
        state,
        interval: tokio::time::interval(tokio::time::Duration::from_secs(30)),
    };

    Ok(HttpResponse::Ok()
        .content_type("text/event-stream")
        .header("Cache-Control", "no-cache")
        .header("Connection", "keep-alive")
        .header("Access-Control-Allow-Origin", "*")
        .streaming(stream))
}

async fn price_stream(state: web::Data<AppState>) -> Result<HttpResponse> {
    let stream = PriceStream {
        state,
        interval: tokio::time::interval(tokio::time::Duration::from_secs(1)), // Update every second
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
    <script src="https://cdn.jsdelivr.net/npm/chart.js"></script>
    <script src="https://cdn.jsdelivr.net/npm/chartjs-adapter-date-fns"></script>
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
        
        .price-chart canvas {
            width: 100% !important;
            height: 100% !important;
        }

        .chart-legend {
            display: flex;
            justify-content: center;
            gap: 20px;
            margin-bottom: 15px;
            padding: 10px;
            background: linear-gradient(145deg, #0a0a0a, #1a1a1a);
            border-radius: 8px;
            border: 1px solid #333333;
        }

        .legend-item {
            display: flex;
            align-items: center;
            gap: 8px;
            color: #ffffff;
            font-size: 0.9rem;
            font-weight: 500;
        }

        .legend-dot {
            width: 12px;
            height: 12px;
            border-radius: 50%;
            border: 2px solid #0a0a0a;
            box-shadow: 0 0 8px rgba(0, 0, 0, 0.3);
        }

        .buy-dot {
            background: #14f195;
            box-shadow: 0 0 8px rgba(20, 241, 149, 0.4);
        }

        .sell-dot {
            background: #ff6b6b;
            box-shadow: 0 0 8px rgba(255, 107, 107, 0.4);
        }

        .hold-dot {
            background: #9945ff;
            box-shadow: 0 0 8px rgba(153, 69, 255, 0.4);
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

            <!-- Price Chart -->
            <div class="card price-chart">
                <h3>📊 Price Chart (24h)</h3>
                <div class="chart-legend">
                    <div class="legend-item">
                        <span class="legend-dot buy-dot"></span>
                        <span>BUY Signal</span>
                    </div>
                    <div class="legend-item">
                        <span class="legend-dot sell-dot"></span>
                        <span>SELL Signal</span>
                    </div>
                    <div class="legend-item">
                        <span class="legend-dot hold-dot"></span>
                        <span>HOLD Signal</span>
                    </div>
                </div>
                <canvas id="priceChart"></canvas>
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
        let priceChart = null;
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
                updatePricePerformance(data.latest_prices, data.performance, data.price_changes);
                updateTechnicalIndicators(data.latest_indicators);
                updateLatestSignals(data.latest_signals, data.market_sentiment, data.price_changes);
                updateActivePositions(data.active_positions);
                updateRecentTrades(data.recent_trades);
                updatePerformanceMetrics(data.performance);
                updatePriceChart(data.price_history, data.latest_signals);

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
                <div class="status-item">
                    <div>📊 Active Positions</div>
                    <div>${status.active_positions}</div>
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

        function updatePricePerformance(prices, performance, priceChanges) {
            const latestPrice = prices[0];
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
                        <span>Current Price:</span>
                        <span class="metric-value" id="current-price" style="position: relative;">
                            $${latestPrice.price.toFixed(4)}
                            <span style="color: #14f195; font-size: 0.8rem; margin-left: 8px; animation: pulse 1s ease-in-out infinite;">●</span>
                        </span>
                    </div>
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
                return `
                    <div class="signal-item signal-${signalClass} ${newClass}" data-signal-id="${signal.id}">
                        <div><strong>${signal.signal_type.toUpperCase()}</strong> - ${(signal.confidence * 100).toFixed(1)}% confidence</div>
                        <div style="display: flex; justify-content: space-between; align-items: center;">
                            <span>Price: $${signal.price.toFixed(4)}</span>
                            <div style="display: flex; gap: 10px; font-size: 0.8rem;">
                                ${priceChangeSpans}
                            </div>
                        </div>
                        <div>${signal.reasoning}</div>
                        <div><small>${new Date(signal.timestamp).toLocaleString()}</small></div>
                    </div>
                `;
            }).join('');

            // Add notification sound for new signals (if supported)
            if (newSignals.length > 0) {
                console.log(`🎯 New signal detected: ${newSignals[0].signal_type.toUpperCase()}`);
                
                // Show notification
                showSignalNotification(newSignals[0]);
                
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
        
        function showSignalNotification(signal) {
            // Create notification element
            const notification = document.createElement('div');
            notification.className = 'signal-notification';
            
            const signalColor = signal.signal_type.toLowerCase() === 'buy' ? '#14f195' : 
                               signal.signal_type.toLowerCase() === 'sell' ? '#ff6b6b' : '#9945ff';
            const signalEmoji = signal.signal_type.toLowerCase() === 'buy' ? '📈' : 
                               signal.signal_type.toLowerCase() === 'sell' ? '📉' : '⏸️';
            
            notification.innerHTML = `
                <div class="notification-content" style="display: flex; align-items: center; gap: 15px;">
                    <div class="notification-icon" style="font-size: 2rem;">${signalEmoji}</div>
                    <div class="notification-text">
                        <div style="font-size: 1.1rem; font-weight: bold; margin-bottom: 5px; color: ${signalColor};">
                            New ${signal.signal_type.toUpperCase()} Signal!
                        </div>
                        <div style="font-size: 0.9rem; opacity: 0.8;">
                            ${(signal.confidence * 100).toFixed(1)}% confidence
                        </div>
                        <div style="font-size: 0.8rem; opacity: 0.6; margin-top: 3px;">
                            $${signal.price.toFixed(4)}
                        </div>
                    </div>
                </div>
            `;
            
            // Add notification styles
            notification.style.cssText = `
                position: fixed;
                top: 100px;
                right: 20px;
                background: linear-gradient(145deg, #0a0a0a, #1a1a1a);
                border: 2px solid ${signalColor};
                border-radius: 15px;
                padding: 20px;
                color: white;
                z-index: 10000;
                transform: translateX(400px);
                transition: transform 0.5s ease-in-out;
                box-shadow: 0 10px 30px rgba(0,0,0,0.8), 0 0 20px ${signalColor}40;
                max-width: 350px;
                backdrop-filter: blur(10px);
            `;
            
            document.body.appendChild(notification);
            
            // Animate in
            setTimeout(() => {
                notification.style.transform = 'translateX(0)';
            }, 100);
            
            // Add pulsing effect
            let pulseCount = 0;
            const pulseInterval = setInterval(() => {
                pulseCount++;
                if (pulseCount >= 6) {
                    clearInterval(pulseInterval);
                    return;
                }
                notification.style.boxShadow = pulseCount % 2 === 0 ? 
                    `0 10px 30px rgba(0,0,0,0.8), 0 0 20px ${signalColor}40` :
                    `0 10px 30px rgba(0,0,0,0.8), 0 0 30px ${signalColor}80`;
            }, 500);
            
            // Remove after 6 seconds
            setTimeout(() => {
                notification.style.transform = 'translateX(400px)';
                setTimeout(() => {
                    if (notification.parentNode) {
                        notification.parentNode.removeChild(notification);
                    }
                }, 500);
            }, 6000);
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

        function updatePriceChart(priceHistory, signals = []) {
            console.log('updatePriceChart called with', priceHistory.length, 'price points');
            
            if (!priceHistory || priceHistory.length === 0) {
                console.error('No price history data provided');
                return;
            }
            
            const canvas = document.getElementById('priceChart');
            if (!canvas) {
                console.error('Price chart canvas not found');
                return;
            }
            
            console.log('Canvas found, getting context...');
            const ctx = canvas.getContext('2d');
            
            if (priceChart) {
                console.log('Destroying existing chart');
                priceChart.destroy();
            }

            console.log('Creating chart with', priceHistory.length, 'price points');
            const labels = priceHistory.map(p => new Date(p.timestamp));
            const prices = priceHistory.map(p => p.price);
            
            console.log('Labels:', labels.length, 'Prices:', prices.length);
            console.log('Sample price data:', prices.slice(0, 3));

            // Create gradient background
            const gradient = ctx.createLinearGradient(0, 0, 0, 400);
            gradient.addColorStop(0, 'rgba(153, 69, 255, 0.3)');
            gradient.addColorStop(0.5, 'rgba(20, 241, 149, 0.1)');
            gradient.addColorStop(1, 'rgba(0, 0, 0, 0.05)');

            // Create signal-based dot colors
            const pointColors = priceHistory.map(pricePoint => {
                const priceTime = new Date(pricePoint.timestamp);
                
                // Find the closest signal to this price point (within 5 minutes)
                const closestSignal = signals.find(signal => {
                    const signalTime = new Date(signal.timestamp);
                    const timeDiff = Math.abs(priceTime - signalTime);
                    return timeDiff <= 5 * 60 * 1000; // 5 minutes in milliseconds
                });

                if (closestSignal) {
                    switch (closestSignal.signal_type.toLowerCase()) {
                        case 'buy':
                            return '#14f195'; // Green for buy
                        case 'sell':
                            return '#ff6b6b'; // Red for sell
                        case 'hold':
                            return '#9945ff'; // Purple for hold
                        default:
                            return '#9945ff'; // Default purple
                    }
                }
                
                // Default color for points without signals
                return '#9945ff';
            });

            priceChart = new Chart(ctx, {
                type: 'line',
                data: {
                    labels: labels,
                    datasets: [{
                        label: 'SOL/USDC Price',
                        data: prices,
                        borderColor: '#9945ff',
                        backgroundColor: gradient,
                        borderWidth: 3,
                        fill: true,
                        tension: 0.4,
                        pointBackgroundColor: pointColors,
                        pointBorderColor: '#0a0a0a',
                        pointBorderWidth: 2,
                        pointRadius: 4,
                        pointHoverRadius: 8,
                        pointHoverBackgroundColor: '#14f195',
                        pointHoverBorderColor: '#0a0a0a',
                        pointHoverBorderWidth: 3
                    }]
                },
                options: {
                    responsive: true,
                    maintainAspectRatio: false,
                    plugins: {
                        legend: {
                            display: false
                        },
                        tooltip: {
                            callbacks: {
                                afterBody: function(context) {
                                    const dataIndex = context[0].dataIndex;
                                    const priceTime = new Date(priceHistory[dataIndex].timestamp);
                                    
                                    // Find signal info for this point
                                    const signal = signals.find(s => {
                                        const signalTime = new Date(s.timestamp);
                                        const timeDiff = Math.abs(priceTime - signalTime);
                                        return timeDiff <= 5 * 60 * 1000;
                                    });
                                    
                                    if (signal) {
                                        return [
                                            `Signal: ${signal.signal_type.toUpperCase()}`,
                                            `Confidence: ${(signal.confidence * 100).toFixed(1)}%`,
                                            `Reason: ${signal.reasoning}`
                                        ];
                                    }
                                    return '';
                                }
                            }
                        }
                    },
                    scales: {
                        x: {
                            type: 'time',
                            time: {
                                unit: 'hour'
                            },
                            grid: {
                                color: 'rgba(153, 69, 255, 0.1)',
                                borderColor: 'rgba(153, 69, 255, 0.2)'
                            },
                            ticks: {
                                color: '#9945ff',
                                font: {
                                    size: 12,
                                    weight: '500'
                                }
                            }
                        },
                        y: {
                            beginAtZero: false,
                            grid: {
                                color: 'rgba(20, 241, 149, 0.1)',
                                borderColor: 'rgba(20, 241, 149, 0.2)'
                            },
                            ticks: {
                                color: '#14f195',
                                font: {
                                    size: 12,
                                    weight: '500'
                                }
                            }
                        }
                    }
                }
            });
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
                setInterval(loadDashboard, 30000);
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
        
        // Initial fetch
        updateExchangePrices();
        
        // Function to update the price chart with new data
        function updatePriceChart(priceHistory, signals) {
            console.log('updatePriceChart called with', priceHistory.length, 'price points');
            
            if (!priceHistory || priceHistory.length === 0) {
                console.error('No price history data provided');
                return;
            }
            
            const canvas = document.getElementById('priceChart');
            if (!canvas) {
                console.error('Price chart canvas not found');
                return;
            }
            
            console.log('Canvas found, getting context...');
            const ctx = canvas.getContext('2d');
            
            // Always create a new chart (this is the main chart creation function)
            if (priceChart) {
                console.log('Destroying existing chart');
                priceChart.destroy();
            }

            console.log('Creating chart with', priceHistory.length, 'price points');
            const labels = priceHistory.map(p => new Date(p.timestamp));
            const prices = priceHistory.map(p => p.price);
            
            console.log('Labels:', labels.length, 'Prices:', prices.length);
            console.log('Sample price data:', prices.slice(0, 3));

            // Create gradient background
            const gradient = ctx.createLinearGradient(0, 0, 0, 400);
            gradient.addColorStop(0, 'rgba(153, 69, 255, 0.3)');
            gradient.addColorStop(0.5, 'rgba(20, 241, 149, 0.1)');
            gradient.addColorStop(1, 'rgba(0, 0, 0, 0.05)');

            // Create signal-based dot colors
            const pointColors = priceHistory.map(pricePoint => {
                const priceTime = new Date(pricePoint.timestamp);
                
                // Find the closest signal to this price point (within 5 minutes)
                const closestSignal = signals.find(signal => {
                    const signalTime = new Date(signal.timestamp);
                    const timeDiff = Math.abs(priceTime - signalTime);
                    return timeDiff <= 5 * 60 * 1000; // 5 minutes in milliseconds
                });

                if (closestSignal) {
                    switch (closestSignal.signal_type.toLowerCase()) {
                        case 'buy':
                            return '#14f195'; // Green for buy
                        case 'sell':
                            return '#ff6b6b'; // Red for sell
                        case 'hold':
                            return '#9945ff'; // Purple for hold
                        default:
                            return '#9945ff'; // Default purple
                    }
                }
                
                // Default color for points without signals
                return '#9945ff';
            });

            priceChart = new Chart(ctx, {
                type: 'line',
                data: {
                    labels: labels,
                    datasets: [{
                        label: 'SOL/USDC Price',
                        data: prices,
                        borderColor: '#9945ff',
                        backgroundColor: gradient,
                        borderWidth: 3,
                        fill: true,
                        tension: 0.4,
                        pointBackgroundColor: pointColors,
                        pointBorderColor: '#0a0a0a',
                        pointBorderWidth: 2,
                        pointRadius: 4,
                        pointHoverRadius: 8,
                        pointHoverBackgroundColor: '#14f195',
                        pointHoverBorderColor: '#0a0a0a',
                        pointHoverBorderWidth: 3
                    }]
                },
                options: {
                    responsive: true,
                    maintainAspectRatio: false,
                    plugins: {
                        legend: {
                            display: false
                        },
                        tooltip: {
                            callbacks: {
                                afterBody: function(context) {
                                    const dataIndex = context[0].dataIndex;
                                    const priceTime = new Date(priceHistory[dataIndex].timestamp);
                                    
                                    // Find signal info for this point
                                    const signal = signals.find(s => {
                                        const signalTime = new Date(s.timestamp);
                                        const timeDiff = Math.abs(priceTime - signalTime);
                                        return timeDiff <= 5 * 60 * 1000;
                                    });
                                    
                                    if (signal) {
                                        return [
                                            `Signal: ${signal.signal_type.toUpperCase()}`,
                                            `Confidence: ${(signal.confidence * 100).toFixed(1)}%`,
                                            `Reason: ${signal.reasoning}`
                                        ];
                                    }
                                    return '';
                                }
                            }
                        }
                    },
                    scales: {
                        x: {
                            type: 'time',
                            time: {
                                unit: 'hour'
                            },
                            grid: {
                                color: 'rgba(153, 69, 255, 0.1)',
                                borderColor: 'rgba(153, 69, 255, 0.2)'
                            },
                            ticks: {
                                color: '#9945ff',
                                font: {
                                    size: 12,
                                    weight: '500'
                                }
                            }
                        },
                        y: {
                            beginAtZero: false,
                            grid: {
                                color: 'rgba(20, 241, 149, 0.1)',
                                borderColor: 'rgba(20, 241, 149, 0.2)'
                            },
                            ticks: {
                                color: '#14f195',
                                font: {
                                    size: 12,
                                    weight: '500'
                                }
                            }
                        }
                    }
                }
            });
            
            console.log('Chart created successfully');
        }
        
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
    });

    HttpServer::new(move || {
        App::new()
            .app_data(app_state.clone())
            .route("/", web::get().to(index))
            .route("/api/dashboard", web::get().to(get_dashboard_data))
            .route("/api/dashboard/stream", web::get().to(dashboard_stream))
            .route("/api/price/stream", web::get().to(price_stream))
            .service(Files::new("/static", "./static").show_files_listing())
    })
    .bind(&bind_addr)?
    .run()
    .await
} 