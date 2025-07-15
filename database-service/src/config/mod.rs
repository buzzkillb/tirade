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
        let sqlite_path = env::var("SQLITE_DB_PATH")
            .unwrap_or_else(|_| "../data/trading_bot.db".to_string());
        
        // Format as SQLite connection string
        let database_url = format!("sqlite:{}", sqlite_path);
            
        let port = env::var("PORT")
            .unwrap_or_else(|_| "8080".to_string())
            .parse::<u16>()
            .map_err(|_| DatabaseServiceError::Config("Invalid PORT".to_string()))?;
            
        let max_connections = env::var("MAX_CONNECTIONS")
            .unwrap_or_else(|_| "20".to_string())  // Increased from 5 to 20
            .parse::<u32>()
            .map_err(|_| DatabaseServiceError::Config("Invalid MAX_CONNECTIONS".to_string()))?;

        Ok(Self {
            database_url,
            port,
            max_connections,
        })
    }
} 