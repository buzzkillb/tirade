use crate::config::Config;
use crate::error::{PriceFeedError, Result};
use reqwest::Client;
use serde::Deserialize;
use tracing::{debug, info};

#[derive(Debug, Deserialize)]
struct PythResponse {
    #[serde(rename = "0xef0d8b6fda2ceba41da15d4095d1da392a0d2f8ed0c6c7bc0f4cfac8c280b56d")]
    sol_usd: Option<PythFeed>,
}

#[derive(Debug, Deserialize)]
struct PythFeed {
    price: PythPrice,
}

#[derive(Debug, Deserialize)]
struct PythPrice {
    price: String,
    expo: i64,
}

#[derive(Clone)]
pub struct PythClient {
    client: Client,
    config: Config,
}

impl PythClient {
    pub fn new(config: Config) -> Self {
        Self {
            client: Client::new(),
            config,
        }
    }

    pub async fn fetch_sol_price(&self) -> Result<f64> {
        let url = format!(
            "{}/latest_price_feeds?ids[]={}",
            self.config.pyth_base_url, self.config.pyth_feed_id
        );

        debug!("Fetching SOL price from Pyth: {}", url);

        let response = self.client.get(&url).send().await?;

        if !response.status().is_success() {
            return Err(PriceFeedError::ApiError {
                status: response.status().as_u16(),
                message: format!("Pyth API returned status: {}", response.status()),
            });
        }

        let text = response.text().await?;
        let feeds: Vec<PythFeed> = serde_json::from_str(&text)?;

        let feed = feeds.first().ok_or_else(|| {
            PriceFeedError::InvalidPriceData {
                message: "No price feeds returned".to_string(),
            }
        })?;

        let price_int = feed.price.price.parse::<i64>().map_err(|_| {
            PriceFeedError::InvalidPriceData {
                message: "Invalid price string".to_string(),
            }
        })?;

        let price_float = price_int as f64 * 10_f64.powi(feed.price.expo as i32);

        info!("[PYTH] SOL/USD: ${:.4}", price_float);
        Ok(price_float)
    }
} 