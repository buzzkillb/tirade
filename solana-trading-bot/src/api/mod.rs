use crate::error::{Result, TradingBotError};
use crate::balance_checker::WalletInfo;
use reqwest::Client;
use serde_json::json;

pub struct DatabaseApi {
    client: Client,
    base_url: String,
}

impl DatabaseApi {
    pub fn new(base_url: String) -> Self {
        Self {
            client: Client::new(),
            base_url,
        }
    }
    
    pub async fn store_balance(&self, wallet_info: &WalletInfo) -> Result<()> {
        let url = format!("{}/balances", self.base_url);
        
        let payload = json!({
            "wallet_address": wallet_info.pubkey,
            "sol_balance": wallet_info.sol_balance,
            "usdc_balance": wallet_info.usdc_balance,
            "timestamp": wallet_info.timestamp.to_rfc3339()
        });
        
        let response = self.client
            .post(&url)
            .json(&payload)
            .send()
            .await
            .map_err(|e| TradingBotError::Http(e))?;
            
        if !response.status().is_success() {
            let status = response.status().as_u16();
            let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            return Err(TradingBotError::Api {
                status,
                message: error_text,
            });
        }
        
        Ok(())
    }
    
    pub async fn health_check(&self) -> Result<bool> {
        let url = format!("{}/health", self.base_url);
        
        match self.client.get(&url).send().await {
            Ok(response) => Ok(response.status().is_success()),
            Err(_) => Ok(false),
        }
    }
} 