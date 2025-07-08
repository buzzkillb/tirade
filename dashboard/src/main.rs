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
        .get(&format!("{}/indicators/SOL%2FUSDC/latest", state.database_url))
        .send()
        .await
    {
        if let Ok(indicators) = response.json::<Vec<TechnicalIndicator>>().await {
            dashboard_data.latest_indicators = indicators;
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
        if let Ok(positions) = response.json::<Vec<Position>>().await {
            dashboard_data.active_positions = positions;
            dashboard_data.system_status.active_positions = dashboard_data.active_positions.len() as i64;
        }
    }

    // Fetch recent trades
    if let Ok(response) = client
        .get(&format!("{}/trades/recent", state.database_url))
        .send()
        .await
    {
        if let Ok(trades) = response.json::<Vec<Trade>>().await {
            dashboard_data.recent_trades = trades;
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
        if let Ok(price_history) = response.json::<Vec<PriceData>>().await {
            dashboard_data.price_history = price_history;
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
            if let Some(count) = count_response["count"].as_i64() {
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
            background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
            color: #333;
            min-height: 100vh;
        }

        .container {
            max-width: 1400px;
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

        .grid {
            display: grid;
            grid-template-columns: repeat(auto-fit, minmax(300px, 1fr));
            gap: 20px;
            margin-bottom: 30px;
        }

        .card {
            background: white;
            border-radius: 15px;
            padding: 20px;
            box-shadow: 0 10px 30px rgba(0,0,0,0.1);
            transition: transform 0.3s ease;
        }

        .card:hover {
            transform: translateY(-5px);
        }

        .card h3 {
            color: #667eea;
            margin-bottom: 15px;
            font-size: 1.2rem;
            border-bottom: 2px solid #f0f0f0;
            padding-bottom: 10px;
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
            background: #f8f9fa;
        }

        .status-item.connected {
            background: #d4edda;
            color: #155724;
        }

        .status-item.disconnected {
            background: #f8d7da;
            color: #721c24;
        }

        .metric {
            display: flex;
            justify-content: space-between;
            align-items: center;
            padding: 10px 0;
            border-bottom: 1px solid #eee;
        }

        .metric:last-child {
            border-bottom: none;
        }

        .metric-value {
            font-weight: bold;
            font-size: 1.1rem;
        }

        .positive { color: #28a745; }
        .negative { color: #dc3545; }
        .neutral { color: #6c757d; }

        .price-chart {
            grid-column: 1 / -1;
            height: 400px;
        }

        .signal-item {
            padding: 10px;
            margin: 5px 0;
            border-radius: 8px;
            border-left: 4px solid;
        }

        .signal-buy { background: #d4edda; border-left-color: #28a745; }
        .signal-sell { background: #f8d7da; border-left-color: #dc3545; }
        .signal-hold { background: #fff3cd; border-left-color: #ffc107; }

        .refresh-btn {
            position: fixed;
            bottom: 30px;
            right: 30px;
            background: #667eea;
            color: white;
            border: none;
            border-radius: 50%;
            width: 60px;
            height: 60px;
            font-size: 1.5rem;
            cursor: pointer;
            box-shadow: 0 5px 15px rgba(0,0,0,0.2);
            transition: all 0.3s ease;
        }

        .refresh-btn:hover {
            background: #5a6fd8;
            transform: scale(1.1);
        }

        .loading {
            text-align: center;
            padding: 20px;
            color: #667eea;
        }

        .error {
            background: #f8d7da;
            color: #721c24;
            padding: 15px;
            border-radius: 8px;
            margin: 10px 0;
        }
    </style>
</head>
<body>
    <div class="container">
        <div class="header">
            <h1>🚀 Tirade Trading Dashboard</h1>
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
                updatePriceChart(data.price_history);

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
                        <div><strong>${signal.signal_type.toUpperCase()}</strong> - ${signal.confidence.toFixed(1)}% confidence</div>
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

        function updatePriceChart(priceHistory) {
            const ctx = document.getElementById('priceChart').getContext('2d');
            
            if (priceChart) {
                priceChart.destroy();
            }

            const labels = priceHistory.map(p => new Date(p.timestamp));
            const prices = priceHistory.map(p => p.price);

            priceChart = new Chart(ctx, {
                type: 'line',
                data: {
                    labels: labels,
                    datasets: [{
                        label: 'SOL/USDC Price',
                        data: prices,
                        borderColor: '#667eea',
                        backgroundColor: 'rgba(102, 126, 234, 0.1)',
                        borderWidth: 2,
                        fill: true,
                        tension: 0.4
                    }]
                },
                options: {
                    responsive: true,
                    maintainAspectRatio: false,
                    plugins: {
                        legend: {
                            display: false
                        }
                    },
                    scales: {
                        x: {
                            type: 'time',
                            time: {
                                unit: 'hour'
                            }
                        },
                        y: {
                            beginAtZero: false
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
    
    println!("🚀 Starting Tirade Dashboard on http://localhost:3000");
    println!("📊 Database URL: {}", database_url);

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
    .bind("127.0.0.1:3000")?
    .run()
    .await
} 