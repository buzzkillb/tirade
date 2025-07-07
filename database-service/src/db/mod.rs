use crate::error::{DatabaseServiceError, Result};
use crate::models::{BalanceSnapshot, PriceFeed, Wallet};
use chrono::Utc;
use sqlx::{sqlite::SqlitePoolOptions, SqlitePool, Row};
use tracing::info;
use uuid::Uuid;

pub struct Database {
    pool: SqlitePool,
}

impl Database {
    pub async fn new(database_url: &str, max_connections: u32) -> Result<Self> {
        let pool = SqlitePoolOptions::new()
            .max_connections(max_connections)
            .connect(database_url)
            .await?;

        Ok(Self { pool })
    }

    pub async fn init_schema(&self) -> Result<()> {
        // Create wallets table
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS wallets (
                id TEXT PRIMARY KEY,
                address TEXT UNIQUE NOT NULL,
                created_at DATETIME NOT NULL,
                updated_at DATETIME NOT NULL
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        // Create balance_snapshots table
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS balance_snapshots (
                id TEXT PRIMARY KEY,
                wallet_id TEXT NOT NULL,
                sol_balance REAL NOT NULL,
                usdc_balance REAL NOT NULL,
                timestamp DATETIME NOT NULL,
                FOREIGN KEY (wallet_id) REFERENCES wallets (id)
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        // Create price_feeds table
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS price_feeds (
                id TEXT PRIMARY KEY,
                source TEXT NOT NULL,
                pair TEXT NOT NULL,
                price REAL NOT NULL,
                timestamp DATETIME NOT NULL
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        // Create indexes
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_balance_snapshots_wallet_id ON balance_snapshots(wallet_id)")
            .execute(&self.pool)
            .await?;

        sqlx::query("CREATE INDEX IF NOT EXISTS idx_balance_snapshots_timestamp ON balance_snapshots(timestamp)")
            .execute(&self.pool)
            .await?;

        sqlx::query("CREATE INDEX IF NOT EXISTS idx_price_feeds_pair ON price_feeds(pair)")
            .execute(&self.pool)
            .await?;

        sqlx::query("CREATE INDEX IF NOT EXISTS idx_price_feeds_timestamp ON price_feeds(timestamp)")
            .execute(&self.pool)
            .await?;

        info!("Database schema initialized successfully");
        Ok(())
    }

    pub async fn create_wallet(&self, address: &str) -> Result<Wallet> {
        let id = Uuid::new_v4().to_string();
        let now = Utc::now();

        sqlx::query(
            r#"
            INSERT INTO wallets (id, address, created_at, updated_at)
            VALUES (?, ?, ?, ?)
            "#,
        )
        .bind(&id)
        .bind(address)
        .bind(&now)
        .bind(&now)
        .execute(&self.pool)
        .await?;

        Ok(Wallet {
            id,
            address: address.to_string(),
            created_at: now,
            updated_at: now,
        })
    }

    pub async fn get_wallet_by_address(&self, address: &str) -> Result<Option<Wallet>> {
        let row = sqlx::query(
            "SELECT id, address, created_at, updated_at FROM wallets WHERE address = ?"
        )
        .bind(address)
        .fetch_optional(&self.pool)
        .await?;

        match row {
            Some(row) => Ok(Some(Wallet {
                id: row.try_get("id")?,
                address: row.try_get("address")?,
                created_at: row.try_get("created_at")?,
                updated_at: row.try_get("updated_at")?,
            })),
            None => Ok(None),
        }
    }

    pub async fn store_balance(&self, wallet_id: &str, sol_balance: f64, usdc_balance: f64) -> Result<BalanceSnapshot> {
        let id = Uuid::new_v4().to_string();
        let now = Utc::now();

        sqlx::query(
            r#"
            INSERT INTO balance_snapshots (id, wallet_id, sol_balance, usdc_balance, timestamp)
            VALUES (?, ?, ?, ?, ?)
            "#,
        )
        .bind(&id)
        .bind(wallet_id)
        .bind(sol_balance)
        .bind(usdc_balance)
        .bind(&now)
        .execute(&self.pool)
        .await?;

        Ok(BalanceSnapshot {
            id,
            wallet_id: wallet_id.to_string(),
            sol_balance,
            usdc_balance,
            timestamp: now,
        })
    }

    pub async fn get_wallet_balances(&self, wallet_address: &str) -> Result<Vec<BalanceSnapshot>> {
        let rows = sqlx::query(
            r#"
            SELECT bs.id, bs.wallet_id, bs.sol_balance, bs.usdc_balance, bs.timestamp
            FROM balance_snapshots bs
            JOIN wallets w ON bs.wallet_id = w.id
            WHERE w.address = ?
            ORDER BY bs.timestamp DESC
            LIMIT 100
            "#,
        )
        .bind(wallet_address)
        .fetch_all(&self.pool)
        .await?;

        let balances: Vec<BalanceSnapshot> = rows
            .into_iter()
            .map(|row| BalanceSnapshot {
                id: row.try_get("id").unwrap_or_default(),
                wallet_id: row.try_get("wallet_id").unwrap_or_default(),
                sol_balance: row.try_get("sol_balance").unwrap_or_default(),
                usdc_balance: row.try_get("usdc_balance").unwrap_or_default(),
                timestamp: row.try_get("timestamp").unwrap_or_default(),
            })
            .collect();

        Ok(balances)
    }

    pub async fn store_price(&self, source: &str, pair: &str, price: f64) -> Result<PriceFeed> {
        let id = Uuid::new_v4().to_string();
        let now = Utc::now();

        sqlx::query(
            r#"
            INSERT INTO price_feeds (id, source, pair, price, timestamp)
            VALUES (?, ?, ?, ?, ?)
            "#,
        )
        .bind(&id)
        .bind(source)
        .bind(pair)
        .bind(price)
        .bind(&now)
        .execute(&self.pool)
        .await?;

        Ok(PriceFeed {
            id,
            source: source.to_string(),
            pair: pair.to_string(),
            price,
            timestamp: now,
        })
    }

    pub async fn get_prices(&self, pair: &str) -> Result<Vec<PriceFeed>> {
        let rows = sqlx::query(
            r#"
            SELECT id, source, pair, price, timestamp
            FROM price_feeds
            WHERE pair = ?
            ORDER BY timestamp DESC
            LIMIT 100
            "#,
        )
        .bind(pair)
        .fetch_all(&self.pool)
        .await?;

        let prices: Vec<PriceFeed> = rows
            .into_iter()
            .map(|row| PriceFeed {
                id: row.try_get("id").unwrap_or_default(),
                source: row.try_get("source").unwrap_or_default(),
                pair: row.try_get("pair").unwrap_or_default(),
                price: row.try_get("price").unwrap_or_default(),
                timestamp: row.try_get("timestamp").unwrap_or_default(),
            })
            .collect();

        Ok(prices)
    }

    pub async fn get_price_history(&self, pair: &str, hours: i64) -> Result<Vec<PriceFeed>> {
        let rows = sqlx::query(
            r#"
            SELECT id, source, pair, price, timestamp
            FROM price_feeds
            WHERE pair = ? AND timestamp >= datetime('now', '-' || ? || ' hours')
            ORDER BY timestamp ASC
            "#,
        )
        .bind(pair)
        .bind(hours.to_string())
        .fetch_all(&self.pool)
        .await?;

        let prices: Vec<PriceFeed> = rows
            .into_iter()
            .map(|row| PriceFeed {
                id: row.try_get("id").unwrap_or_default(),
                source: row.try_get("source").unwrap_or_default(),
                pair: row.try_get("pair").unwrap_or_default(),
                price: row.try_get("price").unwrap_or_default(),
                timestamp: row.try_get("timestamp").unwrap_or_default(),
            })
            .collect();

        Ok(prices)
    }

    pub async fn get_latest_price(&self, pair: &str, source: Option<&str>) -> Result<Option<PriceFeed>> {
        let query = if let Some(src) = source {
            sqlx::query(
                r#"
                SELECT id, source, pair, price, timestamp
                FROM price_feeds
                WHERE pair = ? AND source = ?
                ORDER BY timestamp DESC
                LIMIT 1
                "#,
            )
            .bind(pair)
            .bind(src)
        } else {
            sqlx::query(
                r#"
                SELECT id, source, pair, price, timestamp
                FROM price_feeds
                WHERE pair = ?
                ORDER BY timestamp DESC
                LIMIT 1
                "#,
            )
            .bind(pair)
        };

        let row = query.fetch_optional(&self.pool).await?;

        match row {
            Some(row) => Ok(Some(PriceFeed {
                id: row.try_get("id")?,
                source: row.try_get("source")?,
                pair: row.try_get("pair")?,
                price: row.try_get("price")?,
                timestamp: row.try_get("timestamp")?,
            })),
            None => Ok(None),
        }
    }
} 