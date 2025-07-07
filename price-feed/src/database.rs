use crate::config::Config;
use crate::error::{PriceFeedError, Result};
use reqwest::Client;
use serde_json::json;
use tracing::{error, info};

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
} 