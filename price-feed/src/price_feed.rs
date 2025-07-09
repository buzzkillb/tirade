use crate::api::{JupiterClient, PythClient};
use crate::config::Config;
use crate::database::DatabaseClient;
use crate::error::Result;
use std::time::Duration;
use tokio::time;
use tracing::{error, info};
use std::collections::HashMap;
use chrono::{DateTime, Utc};

#[derive(Debug, Clone)]
struct CandleData {
    open: f64,
    high: f64,
    low: f64,
    close: f64,
    volume: f64,
    count: i32,
    start_time: DateTime<Utc>,
}

impl CandleData {
    fn new(price: f64, timestamp: DateTime<Utc>) -> Self {
        Self {
            open: price,
            high: price,
            low: price,
            close: price,
            volume: 0.0, // No volume data from price feeds
            count: 1,
            start_time: timestamp,
        }
    }

    fn update(&mut self, price: f64) {
        self.high = self.high.max(price);
        self.low = self.low.min(price);
        self.close = price;
        self.count += 1;
    }

    fn to_candle(&self, pair: &str, interval: &str) -> crate::models::Candle {
        crate::models::Candle {
            id: uuid::Uuid::new_v4().to_string(),
            pair: pair.to_string(),
            interval: interval.to_string(),
            open: self.open,
            high: self.high,
            low: self.low,
            close: self.close,
            volume: self.volume,
            timestamp: self.start_time,
            created_at: Utc::now(),
        }
    }
}

pub struct PriceFeedService {
    pyth_client: PythClient,
    jupiter_client: JupiterClient,
    database_client: DatabaseClient,
    config: Config,
    // Candle aggregation state
    candle_data: HashMap<String, CandleData>, // key: "pair_interval"
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
            candle_data: HashMap::new(),
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

        // Start candle aggregation task
        let candle_task = tokio::spawn(async move {
            Self::candle_aggregation_loop().await;
        });

        let _ = tokio::join!(pyth_task, jup_task, candle_task);
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

    async fn candle_aggregation_loop() {
        let mut ticker = time::interval(Duration::from_secs(30)); // Check every 30 seconds
        
        loop {
            ticker.tick().await;
            
            // Aggregate candles for different intervals
            if let Err(e) = Self::aggregate_candles("SOL/USDC", "30s").await {
                error!("Failed to aggregate 30s candles: {}", e);
            }
            
            if let Err(e) = Self::aggregate_candles("SOL/USDC", "1m").await {
                error!("Failed to aggregate 1m candles: {}", e);
            }
            
            if let Err(e) = Self::aggregate_candles("SOL/USDC", "5m").await {
                error!("Failed to aggregate 5m candles: {}", e);
            }
        }
    }

    async fn aggregate_candles(pair: &str, interval: &str) -> Result<()> {
        let config = Config::from_env()?;
        let db_client = DatabaseClient::new(&config);
        
        // Get recent prices for the interval
        let now = Utc::now();
        let interval_seconds = match interval {
            "30s" => 30,
            "1m" => 60,
            "5m" => 300,
            _ => return Err(crate::error::PriceFeedError::ConfigError("Invalid interval".to_string())),
        };
        
        let cutoff_time = now - chrono::Duration::seconds(interval_seconds as i64);
        
        // Get prices from the last interval period
        let prices = db_client.get_prices_since(pair, cutoff_time).await?;
        
        if prices.len() < 2 {
            return Ok(()); // Not enough data for meaningful candle
        }
        
        // Create OHLC candle
        let open = prices.first().unwrap().price;
        let close = prices.last().unwrap().price;
        let high = prices.iter().map(|p| p.price).fold(f64::NEG_INFINITY, f64::max);
        let low = prices.iter().map(|p| p.price).fold(f64::INFINITY, f64::min);
        let volume = 0.0; // No volume data from price feeds
        
        // Store the candle
        db_client.store_candle_with_retry(pair, interval, open, high, low, close, volume, 3).await?;
        
        info!("Created {} candle for {}: O={:.4}, H={:.4}, L={:.4}, C={:.4}", 
              interval, pair, open, high, low, close);
        
        Ok(())
    }
} 