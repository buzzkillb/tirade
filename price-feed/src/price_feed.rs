use crate::api::{JupiterClient, PythClient};
use crate::config::Config;
use crate::database::DatabaseClient;
use crate::error::Result;
use std::time::Duration;
use tokio::time;
use tracing::{error, info};

pub struct PriceFeedService {
    pyth_client: PythClient,
    jupiter_client: JupiterClient,
    database_client: DatabaseClient,
    config: Config,
}

impl PriceFeedService {
    pub fn new(config: Config) -> Self {
        let pyth_client = PythClient::new(config.clone());
        let jupiter_client = JupiterClient::new(config.clone());
        let database_client = DatabaseClient::new(&config);

        Self {
            pyth_client,
            jupiter_client,
            database_client,
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
                    info!("Pyth SOL/USD price: ${:.4}", price);
                    
                    // Store in database with retry logic
                    if let Err(e) = Self::store_pyth_price(price).await {
                        error!("Failed to store Pyth price in database: {}", e);
                    }
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
                    info!("Jupiter SOL/USDC price: ${:.4}", price);
                    
                    // Store in database with retry logic
                    if let Err(e) = Self::store_jupiter_price(price).await {
                        error!("Failed to store Jupiter price in database: {}", e);
                    }
                }
                Err(e) => {
                    error!("Failed to fetch Jupiter price: {}", e);
                }
            }
        }
    }

    async fn store_pyth_price(price: f64) -> Result<()> {
        // Create a temporary database client for this operation
        let config = Config::from_env()?;
        let db_client = DatabaseClient::new(&config);
        
        // Store with retry logic (3 attempts) - use SOL/USDC to match dashboard expectations
        db_client.store_price_with_retry("pyth", "SOL/USDC", price, 3).await
    }

    async fn store_jupiter_price(price: f64) -> Result<()> {
        // Create a temporary database client for this operation
        let config = Config::from_env()?;
        let db_client = DatabaseClient::new(&config);
        
        // Store with retry logic (3 attempts)
        db_client.store_price_with_retry("jupiter", "SOL/USDC", price, 3).await
    }
} 