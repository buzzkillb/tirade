use actix_web::{web, App, HttpServer, HttpResponse, Result};
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use std::sync::Arc;
use reqwest::Client;
use std::time::Duration;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PriceFeed {
    pub id: String,
    pub source: String,
    pub pair: String,
    pub price: f64,
    pub timestamp: DateTime<Utc>,
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
pub struct Position {
    pub id: String,
    pub pair: String,
    pub position_type: String,
    pub entry_price: f64,
    pub entry_time: DateTime<Utc>,
    pub quantity: f64,
    pub status: String,
    pub exit_price: Option<f64>,
    pub exit_time: Option<DateTime<Utc>>,
    pub pnl: Option<f64>,
    pub pnl_percent: Option<f64>,
    pub duration_seconds: Option<i64>,
    pub current_price: f64,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    pub data: Option<T>,
    pub message: Option<String>,
}

struct AppState {
    client: Client,
    database_url: String,
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let database_url = std::env::var("DATABASE_URL").unwrap_or_else(|_| "http://localhost:8080".to_string());
    
    let client = Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .unwrap();

    let state = Arc::new(AppState {
        client,
        database_url: database_url.clone(),
    });

    println!("🚀 Starting Dynamic Tirade Dashboard on http://0.0.0.0:3000");
    println!("📊 Database URL: {}", database_url);

    HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(state.clone()))
            .route("/", web::get().to(index))
            .route("/api/price", web::get().to(get_price))
            .route("/api/pnl", web::get().to(get_pnl))
            .route("/api/active_positions", web::get().to(get_active_positions))
            .route("/api/market_analysis", web::get().to(get_market_analysis))
            .route("/api/signals", web::get().to(get_trading_signals))
            .route("/api/trades", web::get().to(get_trades))
    })
    .bind("0.0.0.0:3000")?
    .run()
    .await
}

async fn index() -> Result<HttpResponse> {
    let html = r#"
<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Tirade Trading Dashboard</title>
    <style>
        * {
            margin: 0;
            padding: 0;
            box-sizing: border-box;
        }

        body {
            font-family: 'Segoe UI', Tahoma, Geneva, Verdana, sans-serif;
            background: linear-gradient(135deg, #0f0f23 0%, #1a1a2e 50%, #16213e 100%);
            min-height: 100vh;
            color: #ffffff;
        }

        .container {
            max-width: 1200px;
            margin: 0 auto;
            padding: 20px;
        }

        .header {
            text-align: center;
            margin-bottom: 30px;
            color: white;
        }

        .header h1 {
            font-size: 2.5rem;
            margin-bottom: 10px;
            text-shadow: 2px 2px 4px rgba(0,0,0,0.3);
        }

        .header p {
            font-size: 1.1rem;
            opacity: 0.9;
        }

        .dashboard-grid {
            display: grid;
            grid-template-columns: repeat(auto-fit, minmax(300px, 1fr));
            gap: 20px;
            margin-bottom: 30px;
        }

        .card {
            background: rgba(255, 255, 255, 0.05);
            border-radius: 15px;
            padding: 25px;
            box-shadow: 0 8px 32px rgba(0,0,0,0.3);
            backdrop-filter: blur(10px);
            border: 1px solid rgba(255,255,255,0.1);
            transition: transform 0.3s ease, box-shadow 0.3s ease;
        }

        .card:hover {
            transform: translateY(-5px);
            box-shadow: 0 12px 40px rgba(0,0,0,0.15);
        }

        .card h2 {
            color: #4a5568;
            margin-bottom: 15px;
            font-size: 1.3rem;
            display: flex;
            align-items: center;
            gap: 10px;
        }

        .price-card {
            background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
            color: white;
        }

        .price-card h2 {
            color: white;
        }

        .price-value {
            font-size: 2.5rem;
            font-weight: bold;
            margin-bottom: 10px;
        }

        .price-change {
            font-size: 1rem;
            opacity: 0.9;
        }

        .pnl-card {
            background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
            color: white;
        }

        .pnl-card h2 {
            color: white;
        }

        .pnl-value {
            font-size: 2.5rem;
            font-weight: bold;
            margin-bottom: 10px;
        }

        .pnl-positive {
            color: #48bb78;
        }

        .pnl-negative {
            color: #f56565;
        }

        .trades-card {
            grid-column: 1 / -1;
            background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
            color: white;
        }
        .trades-card h2 {
            color: white;
        }

        .strategy-card {
            grid-column: 1 / -1;
            background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
            color: white;
        }
        .strategy-card h2 {
            color: white;
        }
        .strategy-grid {
            display: grid;
            grid-template-columns: repeat(auto-fit, minmax(200px, 1fr));
            gap: 15px;
        }
        .strategy-item {
            background: rgba(0,0,0,0.4);
            border-radius: 8px;
            padding: 12px;
            text-align: center;
        }
        .strategy-label {
            font-size: 0.8rem;
            opacity: 0.8;
            margin-bottom: 5px;
            color: white;
        }
        .strategy-value {
            font-size: 1rem;
            font-weight: bold;
            color: white;
        }

        .trades-list {
            max-height: 400px;
            overflow-y: auto;
        }

        .trade-item {
            display: flex;
            justify-content: space-between;
            align-items: center;
            padding: 15px;
            margin-bottom: 10px;
            background: rgba(255,255,255,0.1);
            border-radius: 10px;
            border-left: 4px solid #667eea;
        }

        .trade-buy {
            border-left-color: #48bb78;
        }

        .trade-sell {
            border-left-color: #f56565;
        }

        .trade-info {
            flex: 1;
        }

        .trade-type {
            font-weight: bold;
            text-transform: uppercase;
            font-size: 0.9rem;
            margin-bottom: 5px;
        }

        .trade-buy .trade-type {
            color: #48bb78;
        }

        .trade-sell .trade-type {
            color: #f56565;
        }

        .trade-price {
            font-size: 1.1rem;
            font-weight: bold;
            margin-bottom: 5px;
        }

        .trade-quantity {
            font-size: 0.9rem;
            opacity: 0.8;
        }

        .trade-time {
            text-align: right;
            font-size: 0.8rem;
            opacity: 0.7;
        }

        .status-indicator {
            display: inline-block;
            width: 8px;
            height: 8px;
            border-radius: 50%;
            margin-right: 8px;
        }

        .status-live {
            background: #48bb78;
            animation: pulse 2s infinite;
        }

        @keyframes pulse {
            0% { opacity: 1; }
            50% { opacity: 0.5; }
            100% { opacity: 1; }
        }

        .loading {
            text-align: center;
            padding: 20px;
            color: #666;
        }

        .error {
            color: #f56565;
            text-align: center;
            padding: 20px;
        }

        .refresh-info {
            text-align: center;
            color: white;
            margin-top: 20px;
            opacity: 0.8;
            font-size: 0.9rem;
        }

        .active-trades-card {
            background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
            color: white;
        }
        .active-trades-card h2 {
            color: white;
        }
        .active-trades-value {
            font-size: 2rem;
            font-weight: bold;
            margin-bottom: 10px;
        }
        .active-trade-item {
            background: rgba(255,255,255,0.08);
            border-radius: 10px;
            padding: 12px;
            margin-bottom: 8px;
            font-size: 0.9rem;
        }
        .trade-header {
            font-weight: bold;
            margin-bottom: 6px;
            color: white;
        }
        .trade-details {
            display: flex;
            justify-content: space-between;
            align-items: center;
            color: white;
        }
        .pnl-info {
            font-weight: bold;
            font-size: 0.85rem;
        }
        .pnl-positive {
            color: #48bb78;
        }
        .pnl-negative {
            color: #f56565;
        }

        .signals-card {
            grid-column: 1 / -1;
            background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
            color: white;
        }
        .signals-card h2 {
            color: white;
        }
        .signal-item {
            background: rgba(0,0,0,0.4);
            border-radius: 8px;
            padding: 12px 16px;
            margin-bottom: 10px;
            font-size: 0.9rem;
            display: flex;
            justify-content: space-between;
            align-items: flex-start;
            color: white;
        }
        .signal-info {
            flex: 1;
            color: white;
        }
        .signal-metrics {
            text-align: right;
            font-size: 0.8rem;
            color: white;
        }
        .signal-type {
            font-weight: bold;
            text-transform: uppercase;
            margin-bottom: 4px;
            font-size: 1rem;
            color: white;
        }
        .signal-buy .signal-type {
            color: #48bb78;
        }
        .signal-sell .signal-type {
            color: #f56565;
        }
        .signal-hold .signal-type {
            color: #ed8936;
        }
        .signal-confidence {
            font-size: 0.8rem;
            opacity: 0.9;
            margin-bottom: 2px;
            color: white;
        }
        .signal-reasoning {
            font-size: 0.75rem;
            opacity: 0.7;
            margin-top: 4px;
            font-style: italic;
            color: white;
        }
        .signal-time {
            font-size: 0.7rem;
            opacity: 0.7;
            margin-top: 4px;
            color: white;
        }
        .signal-price {
            font-weight: bold;
            margin-bottom: 2px;
            color: white;
        }
        .signal-indicators {
            font-size: 0.7rem;
            opacity: 0.8;
            color: white;
        }
        .signal-ml-prediction {
            font-size: 0.7rem;
            opacity: 0.8;
            margin-top: 2px;
            color: white;
        }

        .market-analysis-card {
            grid-column: 1 / -1;
            background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
            color: white;
        }
        .market-analysis-card h2 {
            color: white;
        }
        .analysis-grid {
            display: grid;
            grid-template-columns: repeat(auto-fit, minmax(200px, 1fr));
            gap: 15px;
            margin-bottom: 20px;
        }
        .analysis-item {
            background: rgba(0,0,0,0.4);
            border-radius: 8px;
            padding: 12px;
            text-align: center;
        }
        .analysis-label {
            font-size: 0.8rem;
            opacity: 0.8;
            margin-bottom: 5px;
            color: white;
        }
        .analysis-value {
            font-size: 1.2rem;
            font-weight: bold;
            color: white;
        }
        .analysis-details {
            background: rgba(0,0,0,0.4);
            border-radius: 8px;
            padding: 12px;
        }
        .detail-row {
            display: flex;
            justify-content: space-between;
            margin-bottom: 8px;
            font-size: 0.9rem;
            color: white;
        }
        .detail-row:last-child {
            margin-bottom: 0;
        }
        .detail-label {
            opacity: 0.8;
            color: white;
        }
        .detail-value {
            font-weight: bold;
            color: white;
        }
        .regime-trending {
            color: #48bb78;
        }
        .regime-consolidating {
            color: #ed8936;
        }
        .regime-volatile {
            color: #f56565;
        }
        .confidence-high {
            color: #48bb78;
        }
        .confidence-medium {
            color: #ed8936;
        }
        .confidence-low {
            color: #f56565;
        }
        .risk-low {
            color: #48bb78;
        }
        .risk-medium {
            color: #ed8936;
        }
        .risk-high {
            color: #f56565;
        }

        @media (max-width: 768px) {
            .container {
                padding: 10px;
            }
            
            .header h1 {
                font-size: 2rem;
            }
            
            .price-value, .pnl-value {
                font-size: 2rem;
            }
            
            .trade-item {
                flex-direction: column;
                align-items: flex-start;
                gap: 10px;
            }
            
            .trade-time {
                text-align: left;
            }

            /* Mobile improvements for trading signals */
            .signal-item {
                flex-direction: column;
                gap: 12px;
                padding: 16px;
            }
            
            .signal-info {
                width: 100%;
            }
            
            .signal-metrics {
                width: 100%;
                text-align: left;
                background: rgba(0,0,0,0.2);
                border-radius: 6px;
                padding: 8px 12px;
            }
            
            .signal-type {
                font-size: 1.1rem;
                margin-bottom: 6px;
            }
            
            .signal-confidence {
                font-size: 0.9rem;
                margin-bottom: 4px;
            }
            
            .signal-price {
                font-size: 1rem;
                margin-bottom: 4px;
            }
            
            .signal-reasoning {
                font-size: 0.8rem;
                margin-top: 6px;
                line-height: 1.3;
            }
            
            .signal-time {
                font-size: 0.75rem;
                margin-top: 6px;
            }
            
            .signal-indicators {
                font-size: 0.75rem;
                margin-bottom: 4px;
            }
            
            .signal-ml-prediction {
                font-size: 0.75rem;
            }

            /* Mobile improvements for market analysis */
            .analysis-grid {
                grid-template-columns: 1fr 1fr;
                gap: 10px;
            }
            
            .analysis-item {
                padding: 10px;
            }
            
            .analysis-label {
                font-size: 0.75rem;
            }
            
            .analysis-value {
                font-size: 1rem;
            }
            
            .detail-row {
                flex-direction: column;
                gap: 4px;
                margin-bottom: 12px;
            }
            
            .detail-label {
                font-size: 0.8rem;
            }
            
            .detail-value {
                font-size: 0.9rem;
            }
            
            /* Mobile improvements for strategy card */
            .strategy-grid {
                grid-template-columns: 1fr 1fr;
                gap: 10px;
            }
            
            .strategy-item {
                padding: 10px;
            }
            
            .strategy-label {
                font-size: 0.75rem;
            }
            
            .strategy-value {
                font-size: 0.9rem;
            }
        }

        @media (max-width: 480px) {
            .analysis-grid {
                grid-template-columns: 1fr;
            }
            
            .strategy-grid {
                grid-template-columns: 1fr;
            }
            
            .signal-item {
                padding: 14px;
            }
            
            .signal-type {
                font-size: 1rem;
            }
            
            .signal-metrics {
                padding: 6px 10px;
            }
        }
    </style>
</head>
<body>
    <div class="container">
        <div class="header">
            <h1>🚀 Tirade Trading Dashboard</h1>
            <p>Real-time SOL trading data and performance</p>
        </div>

        <div class="dashboard-grid">
            <div class="card price-card">
                <h2>💰 SOL Price</h2>
                <div class="price-value" id="sol-price">Loading...</div>
                <div class="price-change" id="price-change">Updating...</div>
            </div>

            <div class="card pnl-card">
                <h2>📈 PnL (USD)</h2>
                <div class="pnl-value" id="pnl-value">Loading...</div>
                <div class="price-change" id="pnl-status">Calculating...</div>
            </div>

            <div class="card active-trades-card">
                <h2>🟢 Active Trades</h2>
                <div class="active-trades-value" id="active-trades-value">Loading...</div>
                <div class="active-trades-list" id="active-trades-list"></div>
            </div>

            <div class="card market-analysis-card">
                <h2>🧠 Market Analysis & ML (Simplified)</h2>
                <div class="analysis-grid">
                    <div class="analysis-item">
                        <div class="analysis-label">Market Regime</div>
                        <div class="analysis-value" id="market-regime">Loading...</div>
                    </div>
                    <div class="analysis-item">
                        <div class="analysis-label">ML Confidence</div>
                        <div class="analysis-value" id="ml-confidence">Loading...</div>
                    </div>
                    <div class="analysis-item">
                        <div class="analysis-label">Win Rate</div>
                        <div class="analysis-value" id="win-rate">Loading...</div>
                    </div>
                    <div class="analysis-item">
                        <div class="analysis-label">Consecutive Losses</div>
                        <div class="analysis-value" id="consecutive-losses">Loading...</div>
                    </div>
                </div>
                <div class="analysis-details">
                    <div class="detail-row">
                        <span class="detail-label">RSI (14):</span>
                        <span class="detail-value" id="rsi-value">-</span>
                        <span class="detail-label">Volatility:</span>
                        <span class="detail-value" id="volatility-value">-</span>
                    </div>
                    <div class="detail-row">
                        <span class="detail-label">SMA 20:</span>
                        <span class="detail-value" id="sma20-value">-</span>
                        <span class="detail-label">SMA 50:</span>
                        <span class="detail-value" id="sma50-value">-</span>
                    </div>
                    <div class="detail-row">
                        <span class="detail-label">Strategy:</span>
                        <span class="detail-value" id="strategy-type">RSI + MA Trend</span>
                        <span class="detail-label">Confidence Threshold:</span>
                        <span class="detail-value" id="confidence-threshold">35%</span>
                    </div>
                </div>
            </div>

            <div class="card signals-card">
                <h2>📡 Trading Signals (RSI + MA)</h2>
                <div class="signals-list" id="signals-list">
                    <div class="loading">Loading signals...</div>
                </div>
            </div>

            <div class="card trades-card">
                <h2>📊 Recent Trades</h2>
                <div class="trades-list" id="trades-list">
                    <div class="loading">Loading trades...</div>
                </div>
            </div>

            <div class="card strategy-card">
                <h2>⚙️ Strategy Status (Option 2)</h2>
                <div class="strategy-grid">
                    <div class="strategy-item">
                        <div class="strategy-label">Active Strategy</div>
                        <div class="strategy-value">RSI + Moving Average Trend</div>
                    </div>
                    <div class="strategy-item">
                        <div class="strategy-label">ML Features</div>
                        <div class="strategy-value">RSI, Win Rate, Losses, Volatility</div>
                    </div>
                    <div class="strategy-item">
                        <div class="strategy-label">Confidence Threshold</div>
                        <div class="strategy-value">35%</div>
                    </div>
                    <div class="strategy-item">
                        <div class="strategy-label">Position Management</div>
                        <div class="strategy-value">Single Position Only</div>
                    </div>
                </div>
            </div>
        </div>

        <div class="refresh-info">
            <span class="status-indicator status-live"></span>
            Auto-refreshing every 5 seconds
        </div>
    </div>

    <script>
        function formatPrice(price) {
            return '$' + parseFloat(price).toFixed(2);
        }

        function formatTime(timestamp) {
            const date = new Date(timestamp);
            return date.toLocaleTimeString();
        }

        function formatDate(timestamp) {
            const date = new Date(timestamp);
            return date.toLocaleDateString();
        }

        function updatePrice() {
            fetch('/api/price')
                .then(response => response.json())
                .then(data => {
                    if (data && data.price) {
                        document.getElementById('sol-price').textContent = formatPrice(data.price);
                        document.getElementById('price-change').textContent = 
                            `Last updated: ${formatTime(data.timestamp)}`;
                    }
                })
                .catch(error => {
                    console.error('Error fetching price:', error);
                    document.getElementById('sol-price').textContent = 'Error';
                });
        }

        function updatePosition() {
            fetch('/api/pnl')
                .then(response => response.json())
                .then(data => {
                    if (data && data.pnl !== undefined) {
                        const pnlElement = document.getElementById('pnl-value');
                        const pnlClass = data.pnl >= 0 ? 'pnl-positive' : 'pnl-negative';
                        pnlElement.textContent = formatPrice(data.pnl);
                        pnlElement.className = `pnl-value ${pnlClass}`;
                        document.getElementById('pnl-status').textContent = data.pnl >= 0 ? 'Profitable' : 'Loss';
                    } else {
                        document.getElementById('pnl-value').textContent = '$0.00';
                        document.getElementById('pnl-status').textContent = 'No closed trades';
                    }
                })
                .catch(error => {
                    console.error('Error fetching pnl:', error);
                    document.getElementById('pnl-value').textContent = 'Error';
                });
        }

        function updateActiveTrades() {
            fetch('/api/active_positions')
                .then(response => response.json())
                .then(data => {
                    const valueElem = document.getElementById('active-trades-value');
                    const listElem = document.getElementById('active-trades-list');
                    if (data && data.length > 0) {
                        valueElem.textContent = `${data.length} open trade${data.length > 1 ? 's' : ''}`;
                        listElem.innerHTML = data.map(pos => {
                            // Calculate current PnL
                            const currentPrice = pos.current_price || pos.entry_price;
                            const entryPrice = pos.entry_price;
                            const quantity = pos.quantity;
                            
                            let pnlPercent, pnlDollar;
                            if (pos.position_type.toLowerCase() === 'long') {
                                pnlPercent = ((currentPrice - entryPrice) / entryPrice) * 100;
                                pnlDollar = (currentPrice - entryPrice) * quantity;
                            } else {
                                pnlPercent = ((entryPrice - currentPrice) / entryPrice) * 100;
                                pnlDollar = (entryPrice - currentPrice) * quantity;
                            }
                            
                            const pnlClass = pnlPercent >= 0 ? 'pnl-positive' : 'pnl-negative';
                            const pnlEmoji = pnlPercent >= 0 ? '📈' : '📉';
                            
                            return `
                                <div class="active-trade-item">
                                    <div class="trade-header">
                                        <b>${pos.position_type.toUpperCase()}</b> @ $${entryPrice.toFixed(2)}
                                    </div>
                                    <div class="trade-details">
                                        <div>Qty: ${quantity.toFixed(4)} SOL</div>
                                        <div class="pnl-info ${pnlClass}">
                                            ${pnlEmoji} ${pnlPercent.toFixed(2)}% ($${Math.abs(pnlDollar).toFixed(2)})
                                        </div>
                                    </div>
                                </div>
                            `;
                        }).join('');
                    } else {
                        valueElem.textContent = 'No active trades';
                        listElem.innerHTML = '';
                    }
                })
                .catch(error => {
                    document.getElementById('active-trades-value').textContent = 'Error';
                    document.getElementById('active-trades-list').innerHTML = '';
                });
        }

        function updateSignals() {
            fetch('/api/signals')
                .then(response => response.json())
                .then(data => {
                    const signalsList = document.getElementById('signals-list');
                    if (data && data.length > 0) {
                        signalsList.innerHTML = data.map(signal => {
                            const signalType = signal.signal_type || signal.type || 'unknown';
                            const confidence = signal.confidence || 0;
                            const timestamp = signal.timestamp || signal.created_at || new Date();
                            const price = signal.price || 0;
                            const reasoning = signal.reasoning || signal.reason || '';
                            const executed = signal.executed || false;
                            const rsi = signal.rsi || signal.indicators?.rsi || null;
                            const sma20 = signal.sma_20 || signal.indicators?.sma_20 || null;
                            const sma50 = signal.sma_50 || signal.indicators?.sma_50 || null;
                            const ml_prediction = signal.ml_prediction || signal.prediction || null;
                            const volatility = signal.volatility || signal.indicators?.volatility || null;
                            
                            // Simplified indicators for Option 2 strategy
                            const indicators = [];
                            if (rsi !== null) indicators.push(`RSI14: ${rsi.toFixed(1)}`);
                            if (sma20 !== null) indicators.push(`SMA20: $${sma20.toFixed(2)}`);
                            if (sma50 !== null) indicators.push(`SMA50: $${sma50.toFixed(2)}`);
                            if (volatility !== null) indicators.push(`Vol: ${(volatility * 100).toFixed(1)}%`);
                            
                            return `
                                <div class="signal-item signal-${signalType.toLowerCase()}">
                                    <div class="signal-info">
                                        <div class="signal-type">${signalType.toUpperCase()}</div>
                                        <div class="signal-confidence">Confidence: ${(confidence * 100).toFixed(1)}% ${executed ? '✅' : '⏳'}</div>
                                        <div class="signal-price">$${price.toFixed(2)}</div>
                                        ${reasoning ? `<div class="signal-reasoning">${reasoning}</div>` : ''}
                                        <div class="signal-time">${formatTime(timestamp)}</div>
                                    </div>
                                    <div class="signal-metrics">
                                        ${indicators.length > 0 ? `<div class="signal-indicators">${indicators.join(' | ')}</div>` : ''}
                                        ${ml_prediction ? `<div class="signal-ml-prediction">ML: ${ml_prediction}</div>` : ''}
                                    </div>
                                </div>
                            `;
                        }).join('');
                    } else {
                        signalsList.innerHTML = '<div class="loading">No recent signals</div>';
                    }
                })
                .catch(error => {
                    console.error('Error fetching signals:', error);
                    document.getElementById('signals-list').innerHTML = 
                        '<div class="error">Error loading signals</div>';
                });
        }

        function updateTrades() {
            fetch('/api/trades')
                .then(response => response.json())
                .then(data => {
                    const tradesList = document.getElementById('trades-list');
                    
                    if (data && data.length > 0) {
                        tradesList.innerHTML = data.map(trade => `
                            <div class="trade-item trade-${trade.trade_type}">
                                <div class="trade-info">
                                    <div class="trade-type">${trade.trade_type.toUpperCase()}</div>
                                    <div class="trade-price">${formatPrice(trade.price)}</div>
                                    <div class="trade-quantity">${trade.quantity.toFixed(4)} SOL</div>
                                </div>
                                <div class="trade-time">
                                    <div>${formatTime(trade.timestamp)}</div>
                                    <div>${formatDate(trade.timestamp)}</div>
                                </div>
                            </div>
                        `).join('');
                    } else {
                        tradesList.innerHTML = '<div class="loading">No trades found</div>';
                    }
                })
                .catch(error => {
                    console.error('Error fetching trades:', error);
                    document.getElementById('trades-list').innerHTML = 
                        '<div class="error">Error loading trades</div>';
                });
        }

        function updateMarketAnalysis() {
            fetch('/api/market_analysis')
                .then(response => response.json())
                .then(data => {
                    console.log('Market analysis data:', data); // Debug log
                    if (data) {
                        // Update main analysis values
                        const regime = data.market_regime || 'Unknown';
                        const regimeElement = document.getElementById('market-regime');
                        regimeElement.textContent = regime;
                        regimeElement.className = `analysis-value regime-${regime.toLowerCase()}`;
                        
                        const confidence = data.ml_confidence || 0;
                        const confidenceElement = document.getElementById('ml-confidence');
                        confidenceElement.textContent = `${(confidence * 100).toFixed(1)}%`;
                        confidenceElement.className = `analysis-value ${confidence > 0.7 ? 'confidence-high' : confidence > 0.4 ? 'confidence-medium' : 'confidence-low'}`;
                        
                        const winRate = data.win_rate || 0;
                        const winRateElement = document.getElementById('win-rate');
                        winRateElement.textContent = `${(winRate * 100).toFixed(1)}%`;
                        
                        const consecutiveLosses = data.consecutive_losses || 0;
                        const consecutiveLossesElement = document.getElementById('consecutive-losses');
                        consecutiveLossesElement.textContent = `${consecutiveLosses}`;
                        
                        // Update detail values
                        document.getElementById('rsi-value').textContent = data.rsi_14 ? `${data.rsi_14.toFixed(1)}` : '-';
                        document.getElementById('volatility-value').textContent = data.volatility ? `${(data.volatility * 100).toFixed(2)}%` : '-';
                        document.getElementById('sma20-value').textContent = data.sma_20 ? `$${data.sma_20.toFixed(2)}` : '-';
                        document.getElementById('sma50-value').textContent = data.sma_50 ? `$${data.sma_50.toFixed(2)}` : '-';
                        document.getElementById('strategy-type').textContent = data.strategy_type || 'RSI + MA Trend';
                        document.getElementById('confidence-threshold').textContent = data.confidence_threshold ? `${(data.confidence_threshold * 100).toFixed(0)}%` : '35%';
                    } else {
                        console.log('No market analysis data received');
                    }
                })
                .catch(error => {
                    console.error('Error fetching market analysis:', error);
                    document.getElementById('market-regime').textContent = 'Error';
                    document.getElementById('ml-confidence').textContent = 'Error';
                    document.getElementById('win-rate').textContent = 'Error';
                    document.getElementById('consecutive-losses').textContent = 'Error';
                });
        }

        // Initial load
        updatePrice();
        updatePosition();
        updateActiveTrades();
        updateMarketAnalysis();
        updateSignals();
        updateTrades();

        // Auto-refresh every 5 seconds
        setInterval(() => {
            updatePrice();
            updatePosition();
            updateActiveTrades();
            updateMarketAnalysis();
            updateSignals();
            updateTrades();
        }, 5000);
    </script>
</body>
</html>
    "#;

    Ok(HttpResponse::Ok().content_type("text/html").body(html))
}

async fn get_price(state: web::Data<Arc<AppState>>) -> Result<HttpResponse> {
    let url = format!("{}/prices/SOL%2FUSDC/latest", state.database_url);
    
    match state.client.get(&url).send().await {
        Ok(response) => {
            if response.status().is_success() {
                match response.json::<ApiResponse<PriceFeed>>().await {
                    Ok(api_response) => {
                        if let Some(price_data) = api_response.data {
                            Ok(HttpResponse::Ok().json(price_data))
                        } else {
                            Ok(HttpResponse::Ok().json(serde_json::json!({
                                "price": 0.0,
                                "timestamp": Utc::now()
                            })))
                        }
                    }
                    Err(_) => {
                        Ok(HttpResponse::Ok().json(serde_json::json!({
                            "price": 0.0,
                            "timestamp": Utc::now()
                        })))
                    }
                }
            } else {
                Ok(HttpResponse::Ok().json(serde_json::json!({
                    "price": 0.0,
                    "timestamp": Utc::now()
                })))
            }
        }
        Err(_) => {
            Ok(HttpResponse::Ok().json(serde_json::json!({
                "price": 0.0,
                "timestamp": Utc::now()
            })))
        }
    }
}

async fn get_pnl(state: web::Data<Arc<AppState>>) -> Result<HttpResponse> {
    let url = format!("{}/performance/metrics", state.database_url);
    match state.client.get(&url).send().await {
        Ok(response) => {
            if response.status().is_success() {
                match response.json::<ApiResponse<serde_json::Value>>().await {
                    Ok(api_response) => {
                        if let Some(metrics) = api_response.data {
                            let total_pnl = metrics.get("total_pnl").and_then(|v| v.as_f64()).unwrap_or(0.0);
                            Ok(HttpResponse::Ok().json(serde_json::json!({ "pnl": total_pnl })))
                        } else {
                            Ok(HttpResponse::Ok().json(serde_json::json!({ "pnl": 0.0 })))
                        }
                    }
                    Err(_) => Ok(HttpResponse::Ok().json(serde_json::json!({ "pnl": 0.0 }))),
                }
            } else {
                Ok(HttpResponse::Ok().json(serde_json::json!({ "pnl": 0.0 })))
            }
        }
        Err(_) => Ok(HttpResponse::Ok().json(serde_json::json!({ "pnl": 0.0 }))),
    }
}

async fn get_active_positions(state: web::Data<Arc<AppState>>) -> Result<HttpResponse> {
    let url = format!("{}/positions/active", state.database_url);
    match state.client.get(&url).send().await {
        Ok(response) => {
            if response.status().is_success() {
                match response.json::<ApiResponse<Vec<Position>>>().await {
                    Ok(api_response) => {
                        if let Some(positions) = api_response.data {
                            Ok(HttpResponse::Ok().json(positions))
                        } else {
                            Ok(HttpResponse::Ok().json(Vec::<Position>::new()))
                        }
                    }
                    Err(_) => Ok(HttpResponse::Ok().json(Vec::<Position>::new())),
                }
            } else {
                Ok(HttpResponse::Ok().json(Vec::<Position>::new()))
            }
        }
        Err(_) => Ok(HttpResponse::Ok().json(Vec::<Position>::new())),
    }
}

async fn get_trading_signals(state: web::Data<Arc<AppState>>) -> Result<HttpResponse> {
    let url = format!("{}/trading_signals/recent", state.database_url);
    match state.client.get(&url).send().await {
        Ok(response) => {
            if response.status().is_success() {
                match response.json::<ApiResponse<Vec<serde_json::Value>>>().await {
                    Ok(api_response) => {
                        if let Some(signals) = api_response.data {
                            // Take only the last 5 signals and enhance with additional data
                            let mut enhanced_signals: Vec<serde_json::Value> = Vec::new();
                            
                            for signal in signals.into_iter().take(5) {
                                let mut enhanced_signal = signal.clone();
                                
                                // Try to get technical indicators for this signal's timestamp
                                if let Some(timestamp) = signal.get("timestamp").and_then(|t| t.as_str()) {
                                    // For now, we'll add placeholder indicators
                                    // In a real implementation, you'd fetch indicators for the specific timestamp
                                    enhanced_signal["indicators"] = serde_json::json!({
                                        "rsi_14": 65.2,
                                        "sma_20": 162.45,
                                        "sma_50": 161.23,
                                        "volatility": 0.023
                                    });
                                    
                                    // Remove hardcoded ML prediction for simplified Option 2 strategy
                                    // enhanced_signal["ml_prediction"] = serde_json::json!("bullish");
                                }
                                
                                enhanced_signals.push(enhanced_signal);
                            }
                            
                            Ok(HttpResponse::Ok().json(enhanced_signals))
                        } else {
                            Ok(HttpResponse::Ok().json(Vec::<serde_json::Value>::new()))
                        }
                    }
                    Err(_) => Ok(HttpResponse::Ok().json(Vec::<serde_json::Value>::new())),
                }
            } else {
                Ok(HttpResponse::Ok().json(Vec::<serde_json::Value>::new()))
            }
        }
        Err(_) => Ok(HttpResponse::Ok().json(Vec::<serde_json::Value>::new())),
    }
}

async fn get_trades(state: web::Data<Arc<AppState>>) -> Result<HttpResponse> {
    let url = format!("{}/trades/recent", state.database_url);
    
    match state.client.get(&url).send().await {
        Ok(response) => {
            if response.status().is_success() {
                match response.json::<ApiResponse<Vec<Trade>>>().await {
                    Ok(api_response) => {
                        if let Some(trades) = api_response.data {
                            Ok(HttpResponse::Ok().json(trades))
                        } else {
                            Ok(HttpResponse::Ok().json(Vec::<Trade>::new()))
                        }
                    }
                    Err(_) => {
                        Ok(HttpResponse::Ok().json(Vec::<Trade>::new()))
                    }
                }
            } else {
                Ok(HttpResponse::Ok().json(Vec::<Trade>::new()))
            }
        }
        Err(_) => {
            Ok(HttpResponse::Ok().json(Vec::<Trade>::new()))
        }
    }
}

async fn get_market_analysis(state: web::Data<Arc<AppState>>) -> Result<HttpResponse> {
    // Get current price first
    let price_url = format!("{}/prices/SOL%2FUSDC/latest", state.database_url);
    let current_price = match state.client.get(&price_url).send().await {
        Ok(response) => {
            if response.status().is_success() {
                match response.json::<ApiResponse<PriceFeed>>().await {
                    Ok(api_response) => {
                        if let Some(price_data) = api_response.data {
                            price_data.price
                        } else {
                            163.0 // fallback price
                        }
                    }
                    Err(_) => 163.0
                }
            } else {
                163.0
            }
        }
        Err(_) => 163.0
    };

    // Get technical indicators
    let indicators_url = format!("{}/indicators/SOL%2FUSDC", state.database_url);
    let indicators = match state.client.get(&indicators_url).send().await {
        Ok(response) => {
            if response.status().is_success() {
                match response.json::<ApiResponse<serde_json::Value>>().await {
                    Ok(api_response) => {
                        if let Some(indicator_data) = api_response.data {
                            indicator_data
                        } else {
                            serde_json::json!({})
                        }
                    }
                    Err(_) => serde_json::json!({})
                }
            } else {
                serde_json::json!({})
            }
        }
        Err(_) => serde_json::json!({})
    };

    // Get recent positions for ML confidence calculation (positions have PnL data)
    // Use the same query as query_trades.sh - get all positions, not filtered by wallet
    let positions_url = format!("{}/positions/all", state.database_url);
    let recent_positions = match state.client.get(&positions_url).send().await {
        Ok(response) => {
            if response.status().is_success() {
                match response.json::<ApiResponse<Vec<Position>>>().await {
                    Ok(api_response) => {
                        if let Some(positions) = api_response.data {
                            positions
                        } else {
                            vec![]
                        }
                    }
                    Err(_) => vec![]
                }
            } else {
                vec![]
            }
        }
        Err(_) => vec![]
    };

    // Calculate ML confidence based on recent position performance (using actual PnL)
    let ml_confidence = if recent_positions.len() >= 3 {
        let winning_positions = recent_positions.iter()
            .filter(|p| p.status == "closed" && p.pnl.is_some() && p.pnl.unwrap() > 0.0)
            .count();
        let closed_positions = recent_positions.iter()
            .filter(|p| p.status == "closed")
            .count();
        
        if closed_positions > 0 {
            (winning_positions as f64 / closed_positions as f64).min(0.95)
        } else {
            0.65 // Default confidence if not enough data
        }
    } else {
        0.65 // Default confidence if not enough data
    };

    // Get simplified indicators for Option 2 strategy
    let rsi = indicators.get("rsi_14").and_then(|v| v.as_f64()).unwrap_or(50.0);
    let sma_20 = indicators.get("sma_20").and_then(|v| v.as_f64()).unwrap_or(current_price);
    let sma_50 = indicators.get("sma_50").and_then(|v| v.as_f64()).unwrap_or(current_price);
    let volatility = indicators.get("volatility_24h").and_then(|v| v.as_f64()).unwrap_or(0.02);

    // Simplified market regime based on RSI and volatility
    let market_regime = if volatility > 0.05 {
        "Volatile"
    } else if rsi > 70.0 || rsi < 30.0 {
        "Trending"
    } else {
        "Consolidating"
    };

    // Calculate win rate for ML features (using actual PnL from positions)
    let win_rate = if recent_positions.len() >= 3 {
        let winning_positions = recent_positions.iter()
            .filter(|p| p.status == "closed" && p.pnl.is_some() && p.pnl.unwrap() > 0.0)
            .count();
        let closed_positions = recent_positions.iter()
            .filter(|p| p.status == "closed")
            .count();
        
        // Debug logging
        println!("🔍 Dashboard Debug - Total positions: {}, Closed: {}, Wins: {}", 
                 recent_positions.len(), closed_positions, winning_positions);
        
        if closed_positions > 0 {
            let calculated_win_rate = (winning_positions as f64 / closed_positions as f64).min(0.95);
            println!("🔍 Dashboard Debug - Calculated win rate: {:.1}%", calculated_win_rate * 100.0);
            calculated_win_rate
        } else {
            println!("🔍 Dashboard Debug - No closed positions, using default: 65%");
            0.65 // Default win rate if not enough data
        }
    } else {
        println!("🔍 Dashboard Debug - Not enough positions ({}), using default: 65%", recent_positions.len());
        0.65 // Default win rate if not enough data
    };

    // Calculate consecutive losses for ML features (using actual PnL from positions)
    let consecutive_losses = if recent_positions.len() >= 3 {
        let mut losses = 0;
        let mut current_loss_streak = 0;
        
        // Get closed positions sorted by exit time (most recent first)
        let mut closed_positions: Vec<_> = recent_positions.iter()
            .filter(|p| p.status == "closed" && p.exit_time.is_some())
            .collect();
        closed_positions.sort_by(|a, b| b.exit_time.unwrap().cmp(&a.exit_time.unwrap()));
        
        for position in closed_positions {
            if let Some(pnl) = position.pnl {
                if pnl < 0.0 {
                    current_loss_streak += 1;
                    losses = current_loss_streak.max(losses);
                } else {
                    break; // Stop counting when we hit a win
                }
            }
        }
        losses
    } else {
        0 // Default consecutive losses if not enough data
    };

    let analysis_data = serde_json::json!({
        "market_regime": market_regime,
        "ml_confidence": ml_confidence,
        "win_rate": win_rate,
        "consecutive_losses": consecutive_losses,
        "rsi_14": rsi,  // Updated to show RSI14 explicitly
        "sma_20": sma_20,
        "sma_50": sma_50,
        "volatility": volatility,
        "current_price": current_price
    });

    Ok(HttpResponse::Ok().json(analysis_data))
}