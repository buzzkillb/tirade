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
    ml_status: serde_json::Value,
    neural_insights: serde_json::Value,
}

#[derive(Serialize, Deserialize, Debug)]
struct ActiveTrade {
    wallet: String,
    buy_price: f64,
    sol_amount: f64,
    current_pnl: f64,
    pnl_percentage: f64,
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
    reasoning: String,
    neural_enhanced: bool,
    ml_win_rate: f64,
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
    
    // Fetch SOL price from Pyth price feed via database
    let sol_price = match client.get(&format!("{}/prices/SOL%2FUSDC/latest", database_url)).send().await {
        Ok(response) => {
            if let Ok(api_response) = response.json::<serde_json::Value>().await {
                if let Some(data) = api_response.get("data") {
                    if let Some(price_data) = data.as_object() {
                        price_data.get("price").and_then(|p| p.as_f64()).unwrap_or(0.0)
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
        Err(_) => {
            // Fallback to Coinbase if Pyth price feed is unavailable
            match client.get("https://api.coinbase.com/v2/exchange-rates?currency=SOL").send().await {
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
            }
        }
    };

    // Fetch USDC-based PnL from performance metrics (already includes USDC-based calculation)
    let pnl = match client.get(&format!("{}/performance/metrics", database_url)).send().await {
        Ok(response) => {
            if let Ok(api_response) = response.json::<serde_json::Value>().await {
                if let Some(data) = api_response.get("data") {
                    // The total_pnl field already prioritizes USDC-based calculation over price-based
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

    // Fetch active positions and calculate real-time P&L using current SOL price
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
                            
                            let entry_price = pos.get("entry_price").and_then(|p| p.as_f64()).unwrap_or(0.0);
                            let quantity = pos.get("quantity").and_then(|q| q.as_f64()).unwrap_or(0.0);
                            
                            // Calculate real-time P&L using current SOL price
                            let current_value = sol_price * quantity;
                            let initial_investment = entry_price * quantity;
                            let current_pnl = current_value - initial_investment;
                            
                            // Calculate PnL percentage based on initial investment
                            let pnl_percentage = if initial_investment > 0.0 {
                                (current_pnl / initial_investment) * 100.0
                            } else {
                                0.0
                            };
                            
                            ActiveTrade {
                                wallet: short_wallet,
                                buy_price: entry_price,
                                sol_amount: quantity,
                                current_pnl,
                                pnl_percentage,
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

    // Fetch neural network info with enhanced metrics
    let neural_info = match client.get(&format!("{}/neural/performance", database_url)).send().await {
        Ok(response) => {
            if let Ok(neural_data) = response.json::<serde_json::Value>().await {
                // Handle both direct response and wrapped response formats
                let data = neural_data.get("data").unwrap_or(&neural_data);
                
                NeuralInfo {
                    model_accuracy: data.get("overall_accuracy")
                        .or_else(|| data.get("accuracy"))
                        .and_then(|a| a.as_f64()).unwrap_or(0.0),
                    prediction_confidence: data.get("recent_accuracy")
                        .or_else(|| data.get("confidence"))
                        .and_then(|c| c.as_f64()).unwrap_or(0.0),
                    learning_rate: data.get("learning_rate")
                        .and_then(|lr| lr.as_f64()).unwrap_or(0.01),
                    total_predictions: data.get("total_predictions")
                        .and_then(|tp| tp.as_u64()).unwrap_or(0) as u32,
                }
            } else {
                NeuralInfo {
                    model_accuracy: 0.0,
                    prediction_confidence: 0.0,
                    learning_rate: 0.01,
                    total_predictions: 0,
                }
            }
        }
        Err(_) => NeuralInfo {
            model_accuracy: 0.0,
            prediction_confidence: 0.0,
            learning_rate: 0.01,
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
                            let reasoning = signal.get("reasoning").and_then(|r| r.as_str()).unwrap_or("").to_string();
                            let neural_enhanced = reasoning.contains("Neural") || reasoning.contains("ML");
                            let ml_win_rate = if reasoning.contains("ML Win Rate:") {
                                reasoning.split("ML Win Rate: ")
                                    .nth(1)
                                    .and_then(|s| s.split('%').next())
                                    .and_then(|s| s.parse::<f64>().ok())
                                    .unwrap_or(0.0) / 100.0
                            } else {
                                0.0
                            };
                            
                            TradingSignal {
                                signal_type: signal.get("signal_type").and_then(|s| s.as_str()).unwrap_or("").to_string(),
                                pair: signal.get("pair").and_then(|p| p.as_str()).unwrap_or("").to_string(),
                                strength: signal.get("confidence").and_then(|c| c.as_f64()).unwrap_or(0.0),
                                timestamp: signal.get("timestamp").and_then(|t| t.as_str())
                                    .and_then(|ts| DateTime::parse_from_rfc3339(ts).ok())
                                    .map(|dt| dt.with_timezone(&Utc))
                                    .unwrap_or_else(|| Utc::now()),
                                reasoning,
                                neural_enhanced,
                                ml_win_rate,
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

    // Fetch recent trades - get closed positions and create BUY/SELL pairs
    let recent_trades = match client.get(&format!("{}/positions/all?limit=10", database_url)).send().await {
        Ok(response) => {
            if let Ok(api_response) = response.json::<serde_json::Value>().await {
                if let Some(data) = api_response.get("data") {
                    if let Some(positions) = data.as_array() {
                        let mut trades = Vec::new();
                        
                        for position in positions.iter() {
                            // Only process closed positions (have exit_price and exit_time)
                            if let (Some(entry_price), Some(exit_price), Some(quantity), Some(entry_time), Some(exit_time)) = (
                                position.get("entry_price").and_then(|p| p.as_f64()),
                                position.get("exit_price").and_then(|p| p.as_f64()),
                                position.get("quantity").and_then(|q| q.as_f64()),
                                position.get("entry_time").and_then(|t| t.as_str()),
                                position.get("exit_time").and_then(|t| t.as_str())
                            ) {
                                let wallet = position.get("wallet_address").and_then(|w| w.as_str()).unwrap_or("").to_string();
                                let short_wallet = if wallet.len() > 8 { 
                                    format!("{}...{}", &wallet[..4], &wallet[wallet.len()-4..])
                                } else { 
                                    wallet 
                                };

                                // Parse timestamps
                                let entry_dt = DateTime::parse_from_rfc3339(entry_time).ok()
                                    .map(|dt| dt.with_timezone(&Utc))
                                    .unwrap_or_else(|| Utc::now());
                                let exit_dt = DateTime::parse_from_rfc3339(exit_time).ok()
                                    .map(|dt| dt.with_timezone(&Utc))
                                    .unwrap_or_else(|| Utc::now());

                                // Add SELL trade (more recent)
                                trades.push(RecentTrade {
                                    trade_type: "SELL".to_string(),
                                    wallet: short_wallet.clone(),
                                    price: exit_price,
                                    amount: quantity,
                                    timestamp: exit_dt,
                                });

                                // Add BUY trade (older)
                                trades.push(RecentTrade {
                                    trade_type: "BUY".to_string(),
                                    wallet: short_wallet,
                                    price: entry_price,
                                    amount: quantity,
                                    timestamp: entry_dt,
                                });
                            }
                        }
                        
                        // Sort by timestamp (newest first) and take top 10
                        trades.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
                        trades.into_iter().take(10).collect()
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

    // Fetch ML status for additional metrics
    let ml_status = match client.get(&format!("{}/ml/status", database_url)).send().await {
        Ok(response) => {
            if let Ok(ml_data) = response.json::<serde_json::Value>().await {
                ml_data.get("data").cloned().unwrap_or_else(|| serde_json::json!({}))
            } else {
                serde_json::json!({})
            }
        }
        Err(_) => serde_json::json!({}),
    };

    // Fetch neural insights
    let neural_insights = match client.get(&format!("{}/neural/insights", database_url)).send().await {
        Ok(response) => {
            if let Ok(insights_data) = response.json::<serde_json::Value>().await {
                insights_data.get("data").cloned().unwrap_or_else(|| serde_json::json!({}))
            } else {
                serde_json::json!({})
            }
        }
        Err(_) => serde_json::json!({}),
    };

    let dashboard_data = DashboardData {
        sol_price,
        pnl,
        active_trades,
        neural_info,
        recent_signals,
        recent_trades,
        ml_status,
        neural_insights,
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