use actix_web::{web, App, HttpServer, Result, HttpResponse};
use actix_files::Files;
use serde::{Deserialize, Serialize};

use chrono::{DateTime, Utc};

#[derive(Serialize, Deserialize, Debug)]
struct DashboardData {
    sol_price: f64,
    pnl: f64,
    active_trades: Vec<ActiveTrade>,
    neural_info: NeuralInfo,
    recent_signals: Vec<TradingSignal>,
    recent_trades: Vec<RecentTrade>,
}

#[derive(Serialize, Deserialize, Debug)]
struct ActiveTrade {
    wallet: String,
    buy_price: f64,
    sol_amount: f64,
    current_pnl: f64,
}

#[derive(Serialize, Deserialize, Debug)]
struct NeuralInfo {
    model_accuracy: f64,
    prediction_confidence: f64,
    learning_rate: f64,
    total_predictions: u32,
}

#[derive(Serialize, Deserialize, Debug)]
struct TradingSignal {
    signal_type: String,
    pair: String,
    strength: f64,
    timestamp: DateTime<Utc>,
}

#[derive(Serialize, Deserialize, Debug)]
struct RecentTrade {
    trade_type: String,
    wallet: String,
    price: f64,
    amount: f64,
    timestamp: DateTime<Utc>,
}

async fn get_dashboard_data() -> Result<HttpResponse> {
    let client = reqwest::Client::new();
    let database_url = std::env::var("DATABASE_SERVICE_URL").unwrap_or_else(|_| "http://localhost:8080".to_string());
    
    // Fetch SOL price from Coinbase API
    let sol_price = match client.get("https://api.coinbase.com/v2/exchange-rates?currency=SOL").send().await {
        Ok(response) => {
            if let Ok(coinbase_data) = response.json::<serde_json::Value>().await {
                if let Some(data) = coinbase_data.get("data") {
                    if let Some(rates) = data.get("rates") {
                        if let Some(usd_rate) = rates.get("USD") {
                            usd_rate.as_str()
                                .and_then(|s| s.parse::<f64>().ok())
                                .unwrap_or(0.0)
                        } else {
                            0.0
                        }
                    } else {
                        0.0
                    }
                } else {
                    0.0
                }
            } else {
                0.0
            }
        }
        Err(_) => 0.0,
    };

    // Fetch performance metrics for PnL
    let pnl = match client.get(&format!("{}/performance/metrics", database_url)).send().await {
        Ok(response) => {
            if let Ok(api_response) = response.json::<serde_json::Value>().await {
                if let Some(data) = api_response.get("data") {
                    data.get("total_pnl").and_then(|p| p.as_f64()).unwrap_or(0.0)
                } else {
                    0.0
                }
            } else {
                0.0
            }
        }
        Err(_) => 0.0,
    };

    // Fetch active positions
    let active_trades = match client.get(&format!("{}/positions/active", database_url)).send().await {
        Ok(response) => {
            if let Ok(api_response) = response.json::<serde_json::Value>().await {
                if let Some(data) = api_response.get("data") {
                    if let Some(positions) = data.as_array() {
                        positions.iter().map(|pos| {
                            let wallet = pos.get("wallet_address").and_then(|w| w.as_str()).unwrap_or("").to_string();
                            let short_wallet = if wallet.len() > 8 { 
                                format!("{}...{}", &wallet[..4], &wallet[wallet.len()-4..])
                            } else { 
                                wallet 
                            };
                            
                            ActiveTrade {
                                wallet: short_wallet,
                                buy_price: pos.get("entry_price").and_then(|p| p.as_f64()).unwrap_or(0.0),
                                sol_amount: pos.get("quantity").and_then(|q| q.as_f64()).unwrap_or(0.0),
                                current_pnl: pos.get("pnl").and_then(|p| p.as_f64()).unwrap_or(0.0),
                            }
                        }).collect()
                    } else {
                        vec![]
                    }
                } else {
                    vec![]
                }
            } else {
                vec![]
            }
        }
        Err(_) => vec![],
    };

    // Fetch neural network info
    let neural_info = match client.get(&format!("{}/neural/performance", database_url)).send().await {
        Ok(response) => {
            if let Ok(api_response) = response.json::<serde_json::Value>().await {
                if let Some(data) = api_response.get("data") {
                    NeuralInfo {
                        model_accuracy: data.get("accuracy").and_then(|a| a.as_f64()).unwrap_or(0.0),
                        prediction_confidence: data.get("confidence").and_then(|c| c.as_f64()).unwrap_or(0.0),
                        learning_rate: data.get("learning_rate").and_then(|lr| lr.as_f64()).unwrap_or(0.001),
                        total_predictions: data.get("total_predictions").and_then(|tp| tp.as_u64()).unwrap_or(0) as u32,
                    }
                } else {
                    NeuralInfo {
                        model_accuracy: 0.0,
                        prediction_confidence: 0.0,
                        learning_rate: 0.001,
                        total_predictions: 0,
                    }
                }
            } else {
                NeuralInfo {
                    model_accuracy: 0.0,
                    prediction_confidence: 0.0,
                    learning_rate: 0.001,
                    total_predictions: 0,
                }
            }
        }
        Err(_) => NeuralInfo {
            model_accuracy: 0.0,
            prediction_confidence: 0.0,
            learning_rate: 0.001,
            total_predictions: 0,
        },
    };

    // Fetch recent trading signals
    let recent_signals = match client.get(&format!("{}/trading_signals/recent", database_url)).send().await {
        Ok(response) => {
            if let Ok(api_response) = response.json::<serde_json::Value>().await {
                if let Some(data) = api_response.get("data") {
                    if let Some(signals) = data.as_array() {
                        signals.iter().take(5).map(|signal| {
                            TradingSignal {
                                signal_type: signal.get("signal_type").and_then(|s| s.as_str()).unwrap_or("").to_string(),
                                pair: signal.get("pair").and_then(|p| p.as_str()).unwrap_or("").to_string(),
                                strength: signal.get("confidence").and_then(|c| c.as_f64()).unwrap_or(0.0),
                                timestamp: signal.get("timestamp").and_then(|t| t.as_str())
                                    .and_then(|ts| DateTime::parse_from_rfc3339(ts).ok())
                                    .map(|dt| dt.with_timezone(&Utc))
                                    .unwrap_or_else(|| Utc::now()),
                            }
                        }).collect()
                    } else {
                        vec![]
                    }
                } else {
                    vec![]
                }
            } else {
                vec![]
            }
        }
        Err(_) => vec![],
    };

    // Fetch recent trades
    let recent_trades = match client.get(&format!("{}/trades/recent", database_url)).send().await {
        Ok(response) => {
            if let Ok(api_response) = response.json::<serde_json::Value>().await {
                if let Some(data) = api_response.get("data") {
                    if let Some(trades) = data.as_array() {
                        trades.iter().take(10).map(|trade| {
                            let wallet = trade.get("wallet_address").and_then(|w| w.as_str()).unwrap_or("").to_string();
                            let short_wallet = if wallet.len() > 8 { 
                                format!("{}...{}", &wallet[..4], &wallet[wallet.len()-4..])
                            } else { 
                                wallet 
                            };
                            
                            RecentTrade {
                                trade_type: trade.get("position_type").and_then(|t| t.as_str()).unwrap_or("").to_string(),
                                wallet: short_wallet,
                                price: trade.get("exit_price").and_then(|p| p.as_f64())
                                    .or_else(|| trade.get("entry_price").and_then(|p| p.as_f64()))
                                    .unwrap_or(0.0),
                                amount: trade.get("quantity").and_then(|a| a.as_f64()).unwrap_or(0.0),
                                timestamp: trade.get("exit_time").and_then(|t| t.as_str())
                                    .or_else(|| trade.get("entry_time").and_then(|t| t.as_str()))
                                    .and_then(|ts| DateTime::parse_from_rfc3339(ts).ok())
                                    .map(|dt| dt.with_timezone(&Utc))
                                    .unwrap_or_else(|| Utc::now()),
                            }
                        }).collect()
                    } else {
                        vec![]
                    }
                } else {
                    vec![]
                }
            } else {
                vec![]
            }
        }
        Err(_) => vec![],
    };

    let dashboard_data = DashboardData {
        sol_price,
        pnl,
        active_trades,
        neural_info,
        recent_signals,
        recent_trades,
    };

    Ok(HttpResponse::Ok().json(dashboard_data))
}

async fn index() -> Result<HttpResponse> {
    let html = include_str!("../static/index.html");
    Ok(HttpResponse::Ok().content_type("text/html").body(html))
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    dotenv::dotenv().ok();
    
    println!("🚀 Starting Terminal Dashboard...");
    println!("📊 Dashboard will be available at http://localhost:3000");
    
    HttpServer::new(|| {
        App::new()
            .route("/", web::get().to(index))
            .route("/api/dashboard", web::get().to(get_dashboard_data))
            .service(Files::new("/static", "./static").show_files_listing())
    })
    .bind("0.0.0.0:3000")?
    .run()
    .await
}