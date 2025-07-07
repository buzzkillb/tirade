use crate::config::Config;
use crate::error::{PriceFeedError, Result};
use reqwest::Client;
use serde::Deserialize;
use tracing::{debug, info, error};

#[derive(Debug, Deserialize)]
struct JupiterQuoteResponse {
    #[serde(rename = "outAmount")]
    out_amount: String,
}

#[derive(Clone)]
pub struct JupiterClient {
    client: Client,
    config: Config,
}

impl JupiterClient {
    pub fn new(config: Config) -> Self {
        Self {
            client: Client::new(),
            config,
        }
    }

    pub async fn fetch_sol_usdc_price(&self) -> Result<f64> {
        let url = format!(
            "{}/quote?inputMint={}&outputMint={}&amount={}&slippageBps={}",
            self.config.jup_base_url,
            self.config.sol_mint,
            self.config.usdc_mint,
            self.config.sol_amount,
            self.config.slippage_bps
        );

        debug!("Fetching SOL/USDC price from Jupiter: {}", url);

        let response = self.client.get(&url).send().await?;

        if !response.status().is_success() {
            return Err(PriceFeedError::ApiError {
                status: response.status().as_u16(),
                message: format!("Jupiter API returned status: {}", response.status()),
            });
        }

        let text = response.text().await?;
        let quote: std::result::Result<JupiterQuoteResponse, serde_json::Error> = serde_json::from_str(&text);
        match quote {
            Ok(quote) => {
                let out_amount = quote.out_amount.parse::<f64>().map_err(|_| {
                    PriceFeedError::InvalidPriceData {
                        message: "Invalid out_amount string".to_string(),
                    }
                })?;
                // Convert from USDC (6 decimals) to actual USDC amount
                let usdc_amount = out_amount / 1_000_000.0;
                info!("[JUP] SOL/USDC: ${:.4}", usdc_amount);
                Ok(usdc_amount)
            }
            Err(e) => {
                error!("Jupiter raw response: {}", text);
                Err(PriceFeedError::JsonError(e))
            }
        }
    }
} 