use actix_web::{web, App, HttpServer, HttpResponse, Result};
use actix_files::Files;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use chrono::{DateTime, Utc};
use reqwest;

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
    };

    // Fetch latest prices
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

    // Fetch price history for chart
    if let Ok(response) = client
        .get(&format!("{}/prices/SOL%2FUSDC/history?hours=24", state.database_url))
        .send()
        .await
    {
        if let Ok(api_response) = response.json::<serde_json::Value>().await {
            if let Some(price_history_array) = api_response["data"].as_array() {
                if let Ok(price_history) = serde_json::from_value::<Vec<PriceData>>(serde_json::Value::Array(price_history_array.clone())) {
                    dashboard_data.price_history = price_history;
                }
            }
        }
    }

    // Check database connection
    if let Ok(response) = client.get(&format!("{}/health", state.database_url)).send().await {
        dashboard_data.system_status.database_connected = response.status().is_success();
    }

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

async fn index() -> Result<HttpResponse> {
    let html = r#"
<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Tirade Trading Dashboard</title>
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

        .error {
            background: linear-gradient(145deg, #2b1a1a, #2e1a1a);
            color: #ff6b6b;
            padding: 15px;
            border-radius: 8px;
            margin: 10px 0;
            border: 1px solid #ff6b6b;
        }
    </style>
</head>
<body>
    <div class="container">
        <div class="header">
            <h1>Tirade Trading Dashboard</h1>
            <p>Real-time Solana Trading Bot Monitoring</p>
        </div>

        <div id="loading" class="loading">
            <h3>Loading dashboard data...</h3>
        </div>

        <div id="error" class="error" style="display: none;"></div>

        <div id="dashboard" style="display: none;">
            <!-- System Status -->
            <div class="card">
                <h3>🔧 System Status</h3>
                <div class="status-grid" id="system-status"></div>
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
        </div>
    </div>

    <button class="refresh-btn" onclick="loadDashboard()">🔄</button>

    <script>
        let priceChart = null;

        async function loadDashboard() {
            try {
                document.getElementById('loading').style.display = 'block';
                document.getElementById('dashboard').style.display = 'none';
                document.getElementById('error').style.display = 'none';

                const response = await fetch('/api/dashboard');
                if (!response.ok) throw new Error('Failed to fetch dashboard data');
                
                const data = await response.json();
                
                updateSystemStatus(data.system_status);
                updatePricePerformance(data.latest_prices, data.performance);
                updateTechnicalIndicators(data.latest_indicators);
                updateLatestSignals(data.latest_signals);
                updateActivePositions(data.active_positions);
                updateRecentTrades(data.recent_trades);
                updatePerformanceMetrics(data.performance);
                updatePriceChart(data.price_history, data.latest_signals);

                document.getElementById('loading').style.display = 'none';
                document.getElementById('dashboard').style.display = 'block';
            } catch (error) {
                console.error('Error loading dashboard:', error);
                document.getElementById('loading').style.display = 'none';
                document.getElementById('error').style.display = 'block';
                document.getElementById('error').textContent = 'Error loading dashboard: ' + error.message;
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

        function updatePricePerformance(prices, performance) {
            const latestPrice = prices[0];
            const container = document.getElementById('price-performance');
            
            if (latestPrice) {
                const priceChange = latestPrice.price_change_percent_24h || 0;
                const changeClass = priceChange > 0 ? 'positive' : priceChange < 0 ? 'negative' : 'neutral';
                const changeSymbol = priceChange > 0 ? '📈' : priceChange < 0 ? '📉' : '➡️';
                
                container.innerHTML = `
                    <div class="metric">
                        <span>Current Price:</span>
                        <span class="metric-value">$${latestPrice.price.toFixed(4)}</span>
                    </div>
                    <div class="metric">
                        <span>24h Change:</span>
                        <span class="metric-value ${changeClass}">${changeSymbol} ${priceChange.toFixed(2)}%</span>
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

        function updateLatestSignals(signals) {
            const container = document.getElementById('latest-signals');
            
            if (signals.length === 0) {
                container.innerHTML = '<p>No signals generated yet</p>';
                return;
            }

            container.innerHTML = signals.slice(0, 5).map(signal => {
                const signalClass = signal.signal_type.toLowerCase();
                const confidenceColor = signal.confidence > 70 ? 'positive' : signal.confidence > 40 ? 'neutral' : 'negative';
                
                return `
                    <div class="signal-item signal-${signalClass}">
                        <div><strong>${signal.signal_type.toUpperCase()}</strong> - ${(signal.confidence * 100).toFixed(1)}% confidence</div>
                        <div>Price: $${signal.price.toFixed(4)}</div>
                        <div>${signal.reasoning}</div>
                        <div><small>${new Date(signal.timestamp).toLocaleString()}</small></div>
                    </div>
                `;
            }).join('');
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
            const ctx = document.getElementById('priceChart').getContext('2d');
            
            if (priceChart) {
                priceChart.destroy();
            }

            const labels = priceHistory.map(p => new Date(p.timestamp));
            const prices = priceHistory.map(p => p.price);

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

        // Auto-refresh every 30 seconds
        setInterval(loadDashboard, 30000);
        
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
            .service(Files::new("/static", "./static").show_files_listing())
    })
    .bind(&bind_addr)?
    .run()
    .await
} 