use crate::error::{DatabaseServiceError, Result};
use std::env;

#[derive(Debug, Clone)]
pub struct Config {
    pub database_url: String,
    pub port: u16,
    pub max_connections: u32,
}

impl Config {
    pub fn from_env() -> Result<Self> {
        let database_url = env::var("DATABASE_URL")
            .unwrap_or_else(|_| "../data/trading_bot.db".to_string());
            
        let port = env::var("PORT")
            .unwrap_or_else(|_| "8080".to_string())
            .parse::<u16>()
            .map_err(|_| DatabaseServiceError::Config("Invalid PORT".to_string()))?;
            
        let max_connections = env::var("MAX_CONNECTIONS")
            .unwrap_or_else(|_| "5".to_string())
            .parse::<u32>()
            .map_err(|_| DatabaseServiceError::Config("Invalid MAX_CONNECTIONS".to_string()))?;

        Ok(Self {
            database_url,
            port,
            max_connections,
        })
    }
} 