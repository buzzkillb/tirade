use actix_web::{web, App, HttpServer, HttpResponse, Result, Error};
use actix_files::Files;
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use reqwest;
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
pub struct DashboardData {
    pub current_price: Option<PriceData>,
    pub total_pnl: f64,
    pub active_positions: Vec<Position>,
    pub recent_trades: Vec<Trade>,
}

struct AppState {
    database_url: String,
    client: reqwest::Client,
}

async fn get_dashboard_data(state: web::Data<AppState>) -> Result<HttpResponse> {
    let dashboard_data = fetch_dashboard_data(&state).await;
    Ok(HttpResponse::Ok().json(dashboard_data))
}

async fn fetch_dashboard_data(state: &web::Data<AppState>) -> DashboardData {
    let mut dashboard_data = DashboardData {
        current_price: None,
        total_pnl: 0.0,
        active_positions: Vec::new(),
        recent_trades: Vec::new(),
    };

    // Fetch current Pyth price
    if let Ok(Some(price)) = fetch_pyth_price(&state.client, &state.database_url).await {
        dashboard_data.current_price = Some(price);
    }

    // Fetch performance metrics for total PnL
    if let Ok(performance) = fetch_performance(&state.client, &state.database_url).await {
        dashboard_data.total_pnl = performance.total_pnl;
    }

    // Fetch active positions
    if let Ok(positions) = fetch_positions(&state.client, &state.database_url).await {
        dashboard_data.active_positions = positions;
    }

    // Fetch recent trades
    if let Ok(trades) = fetch_trades(&state.client, &state.database_url).await {
        dashboard_data.recent_trades = trades;
    }

    dashboard_data
}

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

async fn fetch_performance(client: &reqwest::Client, database_url: &str) -> Result<PerformanceMetrics, reqwest::Error> {
    let response = client
        .get(&format!("{}/performance/metrics", database_url))
        .timeout(Duration::from_secs(5))
        .send()
        .await?;
    
    if let Ok(api_response) = response.json::<serde_json::Value>().await {
        if let Some(metrics_data) = api_response["data"].as_object() {
            let performance = PerformanceMetrics {
                total_trades: metrics_data["total_trades"].as_i64().unwrap_or(0),
                winning_trades: metrics_data["winning_trades"].as_i64().unwrap_or(0),
                losing_trades: metrics_data["losing_trades"].as_i64().unwrap_or(0),
                win_rate: metrics_data["win_rate"].as_f64().unwrap_or(0.0),
                total_pnl: metrics_data["total_pnl"].as_f64().unwrap_or(0.0),
                total_pnl_percent: metrics_data["total_pnl_percent"].as_f64().unwrap_or(0.0),
                avg_trade_pnl: metrics_data["avg_trade_pnl"].as_f64().unwrap_or(0.0),
                max_drawdown: metrics_data["max_drawdown"].as_f64().unwrap_or(0.0),
                sharpe_ratio: metrics_data["sharpe_ratio"].as_f64().unwrap_or(0.0),
                total_volume: metrics_data["total_volume"].as_f64().unwrap_or(0.0),
            };
            return Ok(performance);
        }
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

async fn index() -> Result<HttpResponse> {
    let html = r#"
<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Tirade Dashboard</title>
    <style>
        * {
            margin: 0;
            padding: 0;
            box-sizing: border-box;
        }
        
        body {
            font-family: 'Segoe UI', Tahoma, Geneva, Verdana, sans-serif;
            background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
            min-height: 100vh;
            color: #333;
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
        
        .grid {
            display: grid;
            grid-template-columns: repeat(auto-fit, minmax(300px, 1fr));
            gap: 20px;
            margin-bottom: 30px;
        }
        
        .card {
            background: white;
            border-radius: 15px;
            padding: 25px;
            box-shadow: 0 10px 30px rgba(0,0,0,0.1);
            transition: transform 0.3s ease;
        }
        
        .card:hover {
            transform: translateY(-5px);
        }
        
        .card h2 {
            color: #667eea;
            margin-bottom: 15px;
            font-size: 1.5rem;
            border-bottom: 2px solid #f0f0f0;
            padding-bottom: 10px;
        }
        
        .price-card {
            background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
            color: white;
        }
        
        .price-card h2 {
            color: white;
            border-bottom-color: rgba(255,255,255,0.3);
        }
        
        .pnl-card {
            background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
            color: white;
        }
        
        .pnl-card h2 {
            color: white;
            border-bottom-color: rgba(255,255,255,0.3);
        }
        
        .positions-card {
            background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
            color: white;
        }
        
        .positions-card h2 {
            color: white;
            border-bottom-color: rgba(255,255,255,0.3);
        }
        
        .trades-card {
            background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
            color: white;
        }
        
        .trades-card h2 {
            color: white;
            border-bottom-color: rgba(255,255,255,0.3);
        }
        
        .price-display {
            font-size: 2.5rem;
            font-weight: bold;
            margin: 10px 0;
        }
        
        .price-change {
            font-size: 1.1rem;
            opacity: 0.9;
        }
        
        .pnl-positive {
            color: #28a745;
        }
        
        .pnl-negative {
            color: #dc3545;
        }
        
        .trade-item {
            background: #f8f9fa;
            border-radius: 8px;
            padding: 15px;
            margin-bottom: 10px;
            border-left: 4px solid #667eea;
        }
        
        .trade-buy {
            border-left-color: #28a745;
        }
        
        .trade-sell {
            border-left-color: #dc3545;
        }
        
        .trade-type {
            font-weight: bold;
            margin-bottom: 5px;
        }
        
        .trade-details {
            font-size: 0.9rem;
            color: #666;
        }
        
        .position-item {
            background: #f8f9fa;
            border-radius: 8px;
            padding: 15px;
            margin-bottom: 10px;
            border-left: 4px solid #ffc107;
        }
        
        .refresh-btn {
            background: #667eea;
            color: white;
            border: none;
            padding: 10px 20px;
            border-radius: 25px;
            cursor: pointer;
            font-size: 1rem;
            transition: background 0.3s ease;
        }
        
        .refresh-btn:hover {
            background: #5a6fd8;
        }
        
        .loading {
            text-align: center;
            color: #666;
            font-style: italic;
        }
        
        .error {
            color: #dc3545;
            text-align: center;
            padding: 20px;
        }
    </style>
</head>
<body>
    <div class="container">
        <div class="header">
            <h1>🚀 Tirade Dashboard</h1>
            <p>Real-time trading performance</p>
        </div>
        
        <div class="grid">
            <div class="card price-card">
                <h2>💰 Current Price</h2>
                <div id="price-display" class="price-display">Loading...</div>
                <div id="price-change" class="price-change">Updating...</div>
            </div>
            
            <div class="card pnl-card">
                <h2>📊 Total PnL</h2>
                <div id="total-pnl" class="price-display">Loading...</div>
                <div id="pnl-details">Loading details...</div>
            </div>
        </div>
        
        <div class="grid">
            <div class="card positions-card">
                <h2>📈 Active Positions</h2>
                <div id="active-positions">Loading...</div>
            </div>
            
            <div class="card trades-card">
                <h2>💼 Recent Trades</h2>
                <div id="recent-trades">Loading...</div>
            </div>
        </div>
        
        <div style="text-align: center; margin-top: 30px;">
            <button class="refresh-btn" onclick="loadDashboard()">🔄 Refresh Data</button>
        </div>
    </div>

    <script>
        let priceUpdateInterval;
        
        async function loadDashboard() {
            try {
                const response = await fetch('/api/dashboard');
                const data = await response.json();
                
                updatePrice(data.current_price);
                updatePnL(data.total_pnl);
                updatePositions(data.active_positions);
                updateTrades(data.recent_trades);
                
            } catch (error) {
                console.error('Error loading dashboard:', error);
                document.getElementById('price-display').textContent = 'Error loading data';
            }
        }
        
        function updatePrice(priceData) {
            const priceDisplay = document.getElementById('price-display');
            const priceChange = document.getElementById('price-change');
            
            if (priceData) {
                priceDisplay.textContent = `$${priceData.price.toFixed(4)}`;
                priceChange.textContent = `Last updated: ${new Date(priceData.timestamp).toLocaleTimeString()}`;
            } else {
                priceDisplay.textContent = 'No price data';
                priceChange.textContent = 'Price feed unavailable';
            }
        }
        
        function updatePnL(totalPnl) {
            const pnlDisplay = document.getElementById('total-pnl');
            const pnlDetails = document.getElementById('pnl-details');
            
            const pnlClass = totalPnl >= 0 ? 'pnl-positive' : 'pnl-negative';
            const pnlSymbol = totalPnl >= 0 ? '+' : '';
            
            pnlDisplay.textContent = `${pnlSymbol}$${totalPnl.toFixed(2)}`;
            pnlDisplay.className = `price-display ${pnlClass}`;
            pnlDetails.textContent = `Total PnL from all closed trades`;
            pnlDetails.style.color = 'rgba(255,255,255,0.9)';
        }
        
        function updatePositions(positions) {
            const container = document.getElementById('active-positions');
            
            if (positions.length === 0) {
                container.innerHTML = '<div class="loading">No active positions</div>';
                return;
            }
            
            container.innerHTML = positions.map(position => `
                <div class="position-item">
                    <div class="trade-type">${position.position_type.toUpperCase()} Position</div>
                    <div class="trade-details">
                        Entry: $${position.entry_price.toFixed(4)} | 
                        Quantity: ${position.quantity.toFixed(6)} SOL<br>
                        Current PnL: <span class="${position.pnl >= 0 ? 'pnl-positive' : 'pnl-negative'}">
                            ${position.pnl >= 0 ? '+' : ''}${(position.pnl * 100).toFixed(2)}%
                        </span><br>
                        Status: ${position.status}
                    </div>
                </div>
            `).join('');
        }
        
        function updateTrades(trades) {
            const container = document.getElementById('recent-trades');
            
            if (trades.length === 0) {
                container.innerHTML = '<div class="loading">No recent trades</div>';
                return;
            }
            
            container.innerHTML = trades.map(trade => `
                <div class="trade-item trade-${trade.trade_type}">
                    <div class="trade-type">
                        ${trade.trade_type === 'buy' ? '🟢 BUY' : '🔴 SELL'}
                    </div>
                    <div class="trade-details">
                        Price: $${trade.price.toFixed(4)} | 
                        Quantity: ${trade.quantity.toFixed(6)} SOL<br>
                        Total: $${trade.total_value.toFixed(2)} | 
                        Status: ${trade.status}<br>
                        Time: ${new Date(trade.timestamp).toLocaleString()}
                    </div>
                </div>
            `).join('');
        }
        
        // Start price updates every 10 seconds
        function startPriceUpdates() {
            loadDashboard(); // Initial load
            priceUpdateInterval = setInterval(loadDashboard, 10000); // Update every 10 seconds
        }
        
        // Initialize dashboard
        startPriceUpdates();
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
    
    println!("🚀 Starting Simplified Tirade Dashboard on http://{}", bind_addr);
    println!("📊 Database URL: {}", database_url);
    println!("🌐 External Access: http://YOUR_VM_PUBLIC_IP:{}", bind_port);

    let app_state = web::Data::new(AppState {
        database_url,
        client: reqwest::Client::new(),
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