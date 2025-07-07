use crate::api::{JupiterClient, PythClient};
use crate::config::Config;
use crate::error::Result;
use std::time::Duration;
use tokio::time;
use tracing::{error, info};

pub struct PriceFeedService {
    pyth_client: PythClient,
    jupiter_client: JupiterClient,
    config: Config,
}

impl PriceFeedService {
    pub fn new(config: Config) -> Self {
        let pyth_client = PythClient::new(config.clone());
        let jupiter_client = JupiterClient::new(config.clone());

        Self {
            pyth_client,
            jupiter_client,
            config,
        }
    }

    pub async fn run(&self) -> Result<()> {
        info!("Starting price feed service...");
        info!("Pyth interval: {}s, Jupiter interval: {}s", 
              self.config.pyth_interval_secs, self.config.jup_interval_secs);

        let pyth_client = self.pyth_client.clone();
        let jupiter_client = self.jupiter_client.clone();
        let pyth_interval = self.config.pyth_interval_secs;
        let jup_interval = self.config.jup_interval_secs;

        let pyth_task = tokio::spawn(async move {
            Self::pyth_loop(pyth_client, pyth_interval).await;
        });
        
        let jup_task = tokio::spawn(async move {
            Self::jupiter_loop(jupiter_client, jup_interval).await;
        });

        let _ = tokio::join!(pyth_task, jup_task);
        Ok(())
    }

    async fn pyth_loop(client: PythClient, interval: u64) {
        let mut ticker = time::interval(Duration::from_secs(interval));
        
        loop {
            ticker.tick().await;
            
            match client.fetch_sol_price().await {
                Ok(price) => {
                    // TODO: Store price in database
                    info!("Pyth SOL/USD price: ${:.4}", price);
                }
                Err(e) => {
                    error!("Failed to fetch Pyth price: {}", e);
                }
            }
        }
    }

    async fn jupiter_loop(client: JupiterClient, interval: u64) {
        let mut ticker = time::interval(Duration::from_secs(interval));
        
        loop {
            ticker.tick().await;
            
            match client.fetch_sol_usdc_price().await {
                Ok(price) => {
                    // TODO: Store price in database
                    info!("Jupiter SOL/USDC price: ${:.4}", price);
                }
                Err(e) => {
                    error!("Failed to fetch Jupiter price: {}", e);
                }
            }
        }
    }
} 