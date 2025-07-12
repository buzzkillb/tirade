use actix_web::{web, App, HttpServer, HttpResponse, Result, Error};
use actix_web::web::Data;
use actix_web_actors::ws;
use actix::{Actor, StreamHandler};
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use regex::Regex;
use anyhow::anyhow;
use tokio::time::interval;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::thread;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct LogEntry {
    pub id: String,
    pub timestamp: DateTime<Utc>,
    pub level: String,
    pub service: String,
    pub message: String,
    pub filtered: bool,
    pub original_length: usize,
    pub filtered_length: usize,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct LogBuffer {
    pub logs: VecDeque<LogEntry>,
    pub max_capacity: usize,
    pub current_size: usize,
}

impl LogBuffer {
    pub fn new(max_capacity: usize) -> Self {
        Self {
            logs: VecDeque::new(),
            max_capacity,
            current_size: 0,
        }
    }

    pub fn add_log(&mut self, log: LogEntry) {
        // Remove logs older than 5 minutes
        let five_minutes_ago = Utc::now() - chrono::Duration::minutes(5);
        
        while let Some(front_log) = self.logs.front() {
            if front_log.timestamp < five_minutes_ago {
                self.logs.pop_front();
                self.current_size = self.current_size.saturating_sub(1);
            } else {
                break;
            }
        }

        // Add new log
        self.logs.push_back(log);
        self.current_size += 1;

        // If we exceed capacity, remove oldest logs
        while self.current_size > self.max_capacity {
            self.logs.pop_front();
            self.current_size = self.current_size.saturating_sub(1);
        }
    }

    pub fn get_recent_logs(&self) -> Vec<LogEntry> {
        self.logs.iter().cloned().collect()
    }
}

#[derive(Clone)]
struct AppState {
    log_buffer: Arc<Mutex<LogBuffer>>,
    running: Arc<AtomicBool>,
}

// WebSocket actor for real-time log streaming
struct LogWebSocket {
    log_buffer: Arc<Mutex<LogBuffer>>,
}

impl Actor for LogWebSocket {
    type Context = ws::WebsocketContext<Self>;
}

impl actix::StreamHandler<Result<ws::Message, ws::ProtocolError>> for LogWebSocket {
    fn handle(&mut self, msg: Result<ws::Message, ws::ProtocolError>, ctx: &mut Self::Context) {
        match msg {
            Ok(ws::Message::Text(text)) => {
                // Send recent logs when client connects
                if text == "get_logs" {
                    if let Ok(buffer) = self.log_buffer.lock() {
                        let logs = buffer.get_recent_logs();
                        for log in logs {
                            if let Ok(log_json) = serde_json::to_string(&log) {
                                ctx.text(log_json);
                            }
                        }
                    }
                }
            }
            Ok(ws::Message::Ping(msg)) => ctx.pong(&msg),
            Ok(ws::Message::Close(reason)) => ctx.close(reason),
            Ok(_) => (),
            Err(e) => {
                eprintln!("WebSocket error: {:?}", e);
            }
        }
    }
}

async fn log_stream(
    req: actix_web::HttpRequest,
    stream: web::Payload,
    log_buffer: Data<Arc<Mutex<LogBuffer>>>,
) -> Result<HttpResponse, Error> {
    let log_buffer = log_buffer.get_ref().clone();
    let ws = LogWebSocket { log_buffer };
    let resp = ws::start(ws, &req, stream)?;
    Ok(resp)
}

// Sensitive data patterns to filter
lazy_static::lazy_static! {
    static ref WALLET_ADDRESS_PATTERN: Regex = Regex::new(r"([1-9A-HJ-NP-Za-km-z]{32,44})").unwrap();
    static ref PRIVATE_KEY_PATTERN: Regex = Regex::new(r"([1-9A-HJ-NP-Za-km-z]{87,88})").unwrap();
    static ref DATABASE_URL_PATTERN: Regex = Regex::new(r"(sqlite|postgres|mysql)://[^\s]+").unwrap();
    static ref API_KEY_PATTERN: Regex = Regex::new(r"(api_key|token|secret)[=:]\s*([^\s]+)").unwrap();
    static ref SOLANA_RPC_PATTERN: Regex = Regex::new(r"(https?://[^\s]+)").unwrap();
}

fn filter_sensitive_data(message: &str) -> (String, bool) {
    let mut filtered_message = message.to_string();
    let mut was_filtered = false;

    // Filter wallet addresses
    if WALLET_ADDRESS_PATTERN.is_match(&filtered_message) {
        filtered_message = WALLET_ADDRESS_PATTERN.replace_all(&filtered_message, "[WALLET_ADDRESS]").to_string();
        was_filtered = true;
    }

    // Filter private keys
    if PRIVATE_KEY_PATTERN.is_match(&filtered_message) {
        filtered_message = PRIVATE_KEY_PATTERN.replace_all(&filtered_message, "[PRIVATE_KEY]").to_string();
        was_filtered = true;
    }

    // Filter database URLs
    if DATABASE_URL_PATTERN.is_match(&filtered_message) {
        filtered_message = DATABASE_URL_PATTERN.replace_all(&filtered_message, "[DATABASE_URL]").to_string();
        was_filtered = true;
    }

    // Filter API keys
    if API_KEY_PATTERN.is_match(&filtered_message) {
        filtered_message = API_KEY_PATTERN.replace_all(&filtered_message, "$1=[API_KEY]").to_string();
        was_filtered = true;
    }

    // Filter Solana RPC URLs (but keep the domain)
    if SOLANA_RPC_PATTERN.is_match(&filtered_message) {
        filtered_message = SOLANA_RPC_PATTERN.replace_all(&filtered_message, "[RPC_URL]").to_string();
        was_filtered = true;
    }

    (filtered_message, was_filtered)
}

async fn add_log(
    state: Data<AppState>,
    log_data: web::Json<LogEntry>,
) -> Result<HttpResponse> {
    let mut log_entry = log_data.into_inner();
    
    // Generate ID if not provided
    if log_entry.id.is_empty() {
        log_entry.id = uuid::Uuid::new_v4().to_string();
    }

    // Filter sensitive data
    let (filtered_message, was_filtered) = filter_sensitive_data(&log_entry.message);
    let original_length = log_entry.message.len();
    
    log_entry.message = filtered_message.clone();
    log_entry.filtered = was_filtered;
    log_entry.original_length = original_length;
    log_entry.filtered_length = filtered_message.len();

    // Add to buffer
    if let Ok(mut buffer) = state.log_buffer.lock() {
        buffer.add_log(log_entry);
    }

    Ok(HttpResponse::Ok().json(serde_json::json!({
        "success": true,
        "message": "Log added successfully"
    })))
}

async fn get_logs(state: Data<AppState>) -> Result<HttpResponse> {
    if let Ok(buffer) = state.log_buffer.lock() {
        let logs = buffer.get_recent_logs();
        Ok(HttpResponse::Ok().json(serde_json::json!({
            "success": true,
            "data": logs
        })))
    } else {
        Ok(HttpResponse::InternalServerError().json(serde_json::json!({
            "success": false,
            "error": "Failed to access log buffer"
        })))
    }
}

async fn index() -> Result<HttpResponse> {
    let html = r#"
    <!DOCTYPE html>
    <html>
    <head>
        <title>Logging Service</title>
        <style>
            body { font-family: Arial, sans-serif; margin: 20px; }
            .status { padding: 10px; margin: 10px 0; border-radius: 5px; }
            .running { background-color: #d4edda; color: #155724; }
            .stopped { background-color: #f8d7da; color: #721c24; }
        </style>
    </head>
    <body>
        <h1>Logging Service</h1>
        <div class="status running">
            ✅ Service is running and collecting logs
        </div>
        <h2>Endpoints:</h2>
        <ul>
            <li><strong>POST /logs</strong> - Add a log entry</li>
            <li><strong>GET /logs</strong> - Get recent logs</li>
            <li><strong>GET /logs/stream</strong> - WebSocket stream</li>
        </ul>
    </body>
    </html>
    "#;
    
    Ok(HttpResponse::Ok()
        .content_type("text/html")
        .body(html))
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    // Initialize logging
    tracing_subscriber::fmt::init();
    
    // Load environment variables
    dotenv::dotenv().ok();
    
    let port = std::env::var("LOGGING_SERVICE_PORT")
        .unwrap_or_else(|_| "8083".to_string())
        .parse::<u16>()
        .unwrap_or(8083);

    let max_logs = std::env::var("MAX_LOG_ENTRIES")
        .unwrap_or_else(|_| "1000".to_string())
        .parse::<usize>()
        .unwrap_or(1000);

    let log_buffer = Arc::new(Mutex::new(LogBuffer::new(max_logs)));
    let running = Arc::new(AtomicBool::new(true));

    let state = AppState {
        log_buffer: log_buffer.clone(),
        running: running.clone(),
    };

    println!("🚀 Starting Logging Service on port {}", port);
    println!("📊 Max log entries: {}", max_logs);
    println!("⏰ Log retention: 5 minutes");

    HttpServer::new(move || {
        App::new()
            .app_data(Data::new(state.clone()))
            .app_data(Data::new(log_buffer.clone()))
            .route("/", web::get().to(index))
            .route("/logs", web::post().to(add_log))
            .route("/logs", web::get().to(get_logs))
            .route("/logs/stream", web::get().to(log_stream))
    })
    .bind(("127.0.0.1", port))?
    .run()
    .await
} 