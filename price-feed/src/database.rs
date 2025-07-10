use crate::config::Config;
use crate::error::{PriceFeedError, Result};
use reqwest::Client;
use serde_json::json;
use tracing::{error, info};
use urlencoding::encode;

pub struct DatabaseClient {
    client: Client,
    base_url: String,
}

impl DatabaseClient {
    pub fn new(config: &Config) -> Self {
        Self {
            client: Client::new(),
            base_url: config.database_url.clone(),
        }
    }

    pub async fn store_price(&self, source: &str, pair: &str, price: f64) -> Result<()> {
        let url = format!("{}/prices", self.base_url);
        
        let payload = json!({
            "source": source,
            "pair": pair,
            "price": price
        });

        let response = self.client
            .post(&url)
            .json(&payload)
            .send()
            .await
            .map_err(|e| PriceFeedError::DatabaseError(format!("Failed to send request: {}", e)))?;

        let status = response.status();
        if !status.is_success() {
            let error_text = response.text().await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(PriceFeedError::DatabaseError(format!("Database API error: {} - {}", status, error_text)));
        }

        info!("Successfully stored {} price: {} = ${:.4}", source, pair, price);
        Ok(())
    }

    pub async fn store_price_with_retry(&self, source: &str, pair: &str, price: f64, max_retries: u32) -> Result<()> {
        let mut attempts = 0;
        
        while attempts < max_retries {
            match self.store_price(source, pair, price).await {
                Ok(_) => return Ok(()),
                Err(e) => {
                    attempts += 1;
                    if attempts >= max_retries {
                        error!("Failed to store {} price after {} attempts: {}", source, max_retries, e);
                        return Err(e);
                    }
                    
                    error!("Failed to store {} price (attempt {}/{}): {}", source, attempts, max_retries, e);
                    
                    // Wait before retrying (exponential backoff)
                    let delay = std::time::Duration::from_secs(2u64.pow(attempts));
                    tokio::time::sleep(delay).await;
                }
            }
        }
        
        Err(PriceFeedError::DatabaseError("Max retries exceeded".to_string()))
    }

    pub async fn get_prices_since(&self, pair: &str, since: chrono::DateTime<chrono::Utc>) -> Result<Vec<crate::models::PriceFeed>> {
        let encoded_pair = encode(pair);
        let url = format!("{}/prices/{}/history?hours={}", 
                         self.base_url, 
                         encoded_pair, 
                         (chrono::Utc::now() - since).num_hours() + 1);
        
        let response = self.client
            .get(&url)
            .send()
            .await
            .map_err(|e| PriceFeedError::DatabaseError(format!("Failed to send request: {}", e)))?;

        let status = response.status();
        if !status.is_success() {
            let error_text = response.text().await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(PriceFeedError::DatabaseError(format!("Database API error: {} - {}", status, error_text)));
        }

        let response_data: crate::models::ApiResponse<Vec<crate::models::PriceFeed>> = response.json().await
            .map_err(|e| PriceFeedError::DatabaseError(format!("Failed to parse response: {}", e)))?;

        match response_data {
            crate::models::ApiResponse { success: true, data: Some(prices), .. } => {
                // Filter prices since the given timestamp
                let filtered_prices: Vec<crate::models::PriceFeed> = prices
                    .into_iter()
                    .filter(|p| p.timestamp >= since)
                    .collect();
                
                Ok(filtered_prices)
            }
            crate::models::ApiResponse { success: false, error: Some(e), .. } => {
                Err(PriceFeedError::DatabaseError(format!("API error: {}", e)))
            }
            _ => {
                Err(PriceFeedError::DatabaseError("Unexpected response format".to_string()))
            }
        }
    }

    pub async fn store_candle(&self, pair: &str, interval: &str, open: f64, high: f64, low: f64, close: f64, volume: f64) -> Result<()> {
        let url = format!("{}/candles", self.base_url);
        
        let payload = json!({
            "pair": pair,
            "interval": interval,
            "open": open,
            "high": high,
            "low": low,
            "close": close,
            "volume": volume
        });

        let response = self.client
            .post(&url)
            .json(&payload)
            .send()
            .await
            .map_err(|e| PriceFeedError::DatabaseError(format!("Failed to send request: {}", e)))?;

        let status = response.status();
        if !status.is_success() {
            let error_text = response.text().await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(PriceFeedError::DatabaseError(format!("Database API error: {} - {}", status, error_text)));
        }

        info!("Successfully stored {} candle for {}: O={:.4}, H={:.4}, L={:.4}, C={:.4}", 
              interval, pair, open, high, low, close);
        Ok(())
    }

    pub async fn store_candle_with_retry(&self, pair: &str, interval: &str, open: f64, high: f64, low: f64, close: f64, volume: f64, max_retries: u32) -> Result<()> {
        let mut attempts = 0;
        
        while attempts < max_retries {
            match self.store_candle(pair, interval, open, high, low, close, volume).await {
                Ok(_) => return Ok(()),
                Err(e) => {
                    attempts += 1;
                    if attempts >= max_retries {
                        error!("Failed to store {} candle after {} attempts: {}", interval, max_retries, e);
                        return Err(e);
                    }
                    
                    error!("Failed to store {} candle (attempt {}/{}): {}", interval, attempts, max_retries, e);
                    
                    // Wait before retrying (exponential backoff)
                    let delay = std::time::Duration::from_secs(2u64.pow(attempts));
                    tokio::time::sleep(delay).await;
                }
            }
        }
        
        Err(PriceFeedError::DatabaseError("Max retries exceeded".to_string()))
    }
} 