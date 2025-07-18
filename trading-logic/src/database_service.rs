use crate::models::{TechnicalIndicators, TradingSignal, SignalType, TechnicalIndicator, TradingSignalDb, PositionDb, TradeDb, StoreTechnicalIndicatorsRequest};
use anyhow::{Result, anyhow};
use reqwest::Client;
use std::time::Duration;
use tracing::{info, warn, error, debug};
use chrono::{DateTime, Utc};
use serde_json;

pub struct DatabaseService {
    client: Client,
    base_url: String,
}

impl DatabaseService {
    // Future enhancement: Add connection pooling for better concurrency
    pub fn new_with_pool(base_url: String, pool_size: usize) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(10))
            .pool_max_idle_per_host(pool_size)
            .build()
            .expect("Failed to create HTTP client with pool");

        Self {
            client,
            base_url,
        }
    }
    pub fn new(base_url: String) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .expect("Failed to create HTTP client");

        Self {
            client,
            base_url,
        }
    }

    pub async fn post_consolidated_indicators(&self, indicators: &TechnicalIndicators, trading_pair: &str) -> Result<()> {
        let url = format!("{}/indicators/{}/store", self.base_url, 
                         urlencoding::encode(trading_pair));
        
        let store_request = StoreTechnicalIndicatorsRequest {
            pair: trading_pair.to_string(),
            sma_20: indicators.sma_20,
            sma_50: indicators.sma_50,
            sma_200: indicators.sma_200,
            rsi_14: indicators.rsi_14,
            price_change_24h: indicators.price_change_24h,
            price_change_percent_24h: indicators.price_change_percent_24h,
            volatility_24h: indicators.volatility_24h,
            current_price: indicators.current_price,
        };
        
        let response = self.client.post(&url)
            .json(&store_request)
            .send()
            .await?;
            
        if !response.status().is_success() {
            warn!("Failed to post consolidated technical indicators: {}", response.status());
        } else {
            debug!("Posted consolidated technical indicators: RSI14={:?}, SMA20={:?}, SMA50={:?}", 
                   indicators.rsi_14, indicators.sma_20, indicators.sma_50);
        }
        
        Ok(())
    }

    pub async fn post_trading_signal(&self, signal: &TradingSignal, trading_pair: &str) -> Result<()> {
        let signal_db = TradingSignalDb {
            pair: trading_pair.to_string(),
            timestamp: signal.timestamp,
            signal_type: match signal.signal_type {
                SignalType::Buy => "buy".to_string(),
                SignalType::Sell => "sell".to_string(),
                SignalType::Hold => "hold".to_string(),
            },
            confidence: signal.confidence,
            price: signal.price,
            reasoning: signal.reasoning.join("; "),
        };

        let url = format!("{}/signals", self.base_url);
        
        let response = self.client.post(&url)
            .json(&signal_db)
            .send()
            .await?;
            
        if !response.status().is_success() {
            warn!("Failed to post trading signal: {}", response.status());
        } else {
            debug!("Posted trading signal: {:?} (confidence: {:.1}%)", signal.signal_type, signal.confidence * 100.0);
        }
        
        Ok(())
    }

    pub async fn create_wallet(&self, wallet_address: &str) -> Result<()> {
        let create_wallet_request = serde_json::json!({
            "address": wallet_address,
        });
        
        let wallet_url = format!("{}/wallets", self.base_url);
        let response = self.client.post(&wallet_url)
            .timeout(Duration::from_secs(10))
            .json(&create_wallet_request)
            .send()
            .await?;
            
        if !response.status().is_success() {
            warn!("Failed to create wallet: {}", response.status());
        }
        
        Ok(())
    }

    pub async fn create_position(&self, wallet_address: &str, trading_pair: &str, position_type: &str, entry_price: f64, quantity: f64) -> Result<String> {
        self.create_position_with_usdc(wallet_address, trading_pair, position_type, entry_price, quantity, None).await
    }

    pub async fn create_position_with_usdc(&self, wallet_address: &str, trading_pair: &str, position_type: &str, entry_price: f64, quantity: f64, usdc_spent: Option<f64>) -> Result<String> {
        let mut create_position_request = serde_json::json!({
            "wallet_address": wallet_address,
            "pair": trading_pair,
            "position_type": position_type,
            "entry_price": entry_price,
            "quantity": quantity,
        });

        // Add USDC spent if provided (actual USDC flow from transaction)
        if let Some(usdc_amount) = usdc_spent {
            create_position_request["usdc_spent"] = serde_json::Value::from(usdc_amount.abs());
            info!("💰 Recording actual USDC spent: ${:.2}", usdc_amount.abs());
        }

        let url = format!("{}/positions", self.base_url);
        
        let response = self.client.post(&url)
            .timeout(Duration::from_secs(15))
            .json(&create_position_request)
            .send()
            .await?;
            
        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            let error_msg = format!("Failed to post position to database: {} - {}", status, error_text);
            error!("❌ {}", error_msg);
            return Err(anyhow!(error_msg));
        }

        let response_text = response.text().await.unwrap_or_else(|_| "No response body".to_string());
        info!("✅ Successfully posted position to database: {} at ${:.4}", position_type, entry_price);
        debug!("📊 Database response: {}", response_text);
        
        // Parse the response to get the position ID
        match serde_json::from_str::<serde_json::Value>(&response_text) {
            Ok(json_response) => {
                if let Some(data) = json_response.get("data") {
                    if let Some(position_id) = data.get("id").and_then(|id| id.as_str()) {
                        info!("🆔 Captured position ID: {}", position_id);
                        return Ok(position_id.to_string());
                    }
                }
                warn!("⚠️ Could not extract position ID from response");
                Ok("".to_string())
            }
            Err(e) => {
                warn!("⚠️ Failed to parse position response: {}", e);
                Ok("".to_string())
            }
        }
    }

    pub async fn close_position(&self, position_id: &str, exit_price: f64) -> Result<()> {
        self.close_position_with_usdc(position_id, exit_price, None).await
    }

    pub async fn close_position_with_usdc(&self, position_id: &str, exit_price: f64, usdc_received: Option<f64>) -> Result<()> {
        let mut close_request = serde_json::json!({
            "position_id": position_id,
            "exit_price": exit_price,
            "transaction_hash": None::<String>,
            "fees": None::<f64>,
        });

        // Add USDC received if provided (actual USDC flow from transaction)
        if let Some(usdc_amount) = usdc_received {
            close_request["usdc_received"] = serde_json::Value::from(usdc_amount.abs());
            info!("💰 Recording actual USDC received: ${:.2}", usdc_amount.abs());
        }

        let close_url = format!("{}/positions/close", self.base_url);
        let response = self.client.post(&close_url)
            .json(&close_request)
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(anyhow!("Failed to close position: {}", response.status()));
        }

        info!("✅ Position closed in database");
        Ok(())
    }

    pub async fn fetch_open_positions_for_wallet(&self, wallet_address: &str) -> Result<Option<PositionDb>> {
        let url = format!("{}/positions/{}/open", self.base_url, 
                         urlencoding::encode(wallet_address));
        
        let response = self.client.get(&url).send().await?;
        let text = response.text().await?;
        
        if text.trim().is_empty() {
            return Ok(None);
        }
        
        let api_response: Result<crate::models::ApiResponse<Vec<PositionDb>>, _> = serde_json::from_str(&text);
        match api_response {
            Ok(crate::models::ApiResponse { success: true, data: Some(positions), .. }) => {
                // Return the first open position if any exist
                Ok(positions.into_iter().next())
            }
            _ => {
                warn!("Failed to parse positions response: {}", text);
                Ok(None)
            }
        }
    }

    pub async fn check_health(&self) -> Result<bool> {
        let health_url = format!("{}/health", self.base_url);
        
        match self.client.get(&health_url)
            .timeout(Duration::from_secs(5))
            .send()
            .await {
            Ok(response) => {
                if response.status().is_success() {
                    debug!("✅ Database health check passed");
                    Ok(true)
                } else {
                    warn!("⚠️ Database health check failed: {}", response.status());
                    Ok(false)
                }
            }
            Err(e) => {
                warn!("⚠️ Database health check error: {}", e);
                Ok(false)
            }
        }
    }

    pub async fn post_trading_config(&self, trading_pair: &str, min_confidence: f64, position_size: f64, slippage: f64) -> Result<()> {
        let create_config_request = serde_json::json!({
            "name": format!("{}_config", trading_pair),
            "pair": trading_pair,
            "min_data_points": 200,
            "check_interval_secs": 30,
            "take_profit_percent": 2.0,
            "stop_loss_percent": 1.4,
            "max_position_size": position_size * 100.0,
        });

        let url = format!("{}/configs", self.base_url);
        
        let response = self.client.post(&url)
            .json(&create_config_request)
            .send()
            .await?;
            
        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            warn!("Failed to post trading config: {} - {}", status, error_text);
            warn!("Request payload: {}", serde_json::to_string_pretty(&create_config_request).unwrap_or_else(|_| "Failed to serialize".to_string()));
            warn!("Request URL: {}", url);
        } else {
            info!("✅ Posted trading config for {} successfully", trading_pair);
        }
        
        Ok(())
    }

    // Neural network performance endpoint
    pub async fn get_neural_performance(&self) -> Result<serde_json::Value> {
        let url = format!("{}/neural/performance", self.base_url);
        
        let response = self.client.get(&url)
            .timeout(Duration::from_secs(5))
            .send()
            .await?;
            
        if !response.status().is_success() {
            // Return default neural performance data if endpoint doesn't exist
            return Ok(serde_json::json!({
                "enabled": false,
                "message": "Neural endpoint not available",
                "learning_rate": 0.01,
                "total_predictions": 0,
                "accuracy": 0.0,
                "confidence": 0.0
            }));
        }

        let neural_data: serde_json::Value = response.json().await?;
        Ok(neural_data)
    }

    // Neural network insights endpoint
    pub async fn get_neural_insights(&self) -> Result<serde_json::Value> {
        let url = format!("{}/neural/insights", self.base_url);
        
        let response = self.client.get(&url)
            .timeout(Duration::from_secs(5))
            .send()
            .await?;
            
        if !response.status().is_success() {
            // Return default insights if endpoint doesn't exist
            return Ok(serde_json::json!({
                "enabled": false,
                "insights": [],
                "message": "Neural insights not available"
            }));
        }

        let insights_data: serde_json::Value = response.json().await?;
        Ok(insights_data)
    }
}