use crate::error::{DatabaseServiceError, Result};
use crate::models::{BalanceSnapshot, PriceFeed, Wallet, MLTradeHistory, MLTradeStats};
use chrono::{DateTime, Utc};
use sqlx::{sqlite::SqlitePoolOptions, SqlitePool, Row};
use tracing::{info, error, warn};
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

        // Create technical_indicators table
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS technical_indicators (
                id TEXT PRIMARY KEY,
                pair TEXT NOT NULL,
                timestamp DATETIME NOT NULL,
                sma_20 REAL,
                sma_50 REAL,
                sma_200 REAL,
                rsi_14 REAL,
                price_change_24h REAL,
                price_change_percent_24h REAL,
                volatility_24h REAL,
                current_price REAL NOT NULL,
                created_at DATETIME NOT NULL
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        // Create trading_signals table
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS trading_signals (
                id TEXT PRIMARY KEY,
                pair TEXT NOT NULL,
                signal_type TEXT NOT NULL,
                confidence REAL NOT NULL,
                price REAL NOT NULL,
                timestamp DATETIME NOT NULL,
                reasoning TEXT,
                executed BOOLEAN DEFAULT FALSE,
                created_at DATETIME NOT NULL
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        // Create positions table
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS positions (
                id TEXT PRIMARY KEY,
                wallet_id TEXT NOT NULL,
                pair TEXT NOT NULL,
                position_type TEXT NOT NULL,
                entry_price REAL NOT NULL,
                entry_time DATETIME NOT NULL,
                quantity REAL NOT NULL,
                status TEXT NOT NULL DEFAULT 'open',
                exit_price REAL,
                exit_time DATETIME,
                pnl REAL,
                pnl_percent REAL,
                duration_seconds INTEGER,
                created_at DATETIME NOT NULL,
                updated_at DATETIME NOT NULL,
                FOREIGN KEY (wallet_id) REFERENCES wallets (id)
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        // Create trades table (detailed trade history)
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS trades (
                id TEXT PRIMARY KEY,
                position_id TEXT NOT NULL,
                trade_type TEXT NOT NULL,
                price REAL NOT NULL,
                quantity REAL NOT NULL,
                timestamp DATETIME NOT NULL,
                transaction_hash TEXT,
                fees REAL,
                created_at DATETIME NOT NULL,
                FOREIGN KEY (position_id) REFERENCES positions (id)
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        // Create trading_configs table
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS trading_configs (
                id TEXT PRIMARY KEY,
                name TEXT UNIQUE NOT NULL,
                pair TEXT NOT NULL,
                min_data_points INTEGER NOT NULL DEFAULT 200,
                check_interval_secs INTEGER NOT NULL DEFAULT 30,
                take_profit_percent REAL NOT NULL DEFAULT 2.0,
                stop_loss_percent REAL NOT NULL DEFAULT 1.4,
                max_position_size REAL NOT NULL DEFAULT 100.0,
                enabled BOOLEAN NOT NULL DEFAULT TRUE,
                created_at DATETIME NOT NULL,
                updated_at DATETIME NOT NULL
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        // Create performance_metrics table
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS performance_metrics (
                id TEXT PRIMARY KEY,
                wallet_id TEXT NOT NULL,
                period TEXT NOT NULL,
                total_trades INTEGER NOT NULL DEFAULT 0,
                winning_trades INTEGER NOT NULL DEFAULT 0,
                losing_trades INTEGER NOT NULL DEFAULT 0,
                total_pnl REAL NOT NULL DEFAULT 0.0,
                total_pnl_percent REAL NOT NULL DEFAULT 0.0,
                win_rate REAL NOT NULL DEFAULT 0.0,
                avg_win REAL NOT NULL DEFAULT 0.0,
                avg_loss REAL NOT NULL DEFAULT 0.0,
                max_drawdown REAL NOT NULL DEFAULT 0.0,
                sharpe_ratio REAL,
                timestamp DATETIME NOT NULL,
                created_at DATETIME NOT NULL,
                FOREIGN KEY (wallet_id) REFERENCES wallets (id)
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        // Create candles table for OHLC data aggregation
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS candles (
                id TEXT PRIMARY KEY,
                pair TEXT NOT NULL,
                interval TEXT NOT NULL,
                open REAL NOT NULL,
                high REAL NOT NULL,
                low REAL NOT NULL,
                close REAL NOT NULL,
                volume REAL NOT NULL DEFAULT 0.0,
                timestamp DATETIME NOT NULL,
                created_at DATETIME NOT NULL,
                UNIQUE(pair, interval, timestamp)
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        // Create ml_trade_history table for ML learning persistence
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS ml_trade_history (
                id TEXT PRIMARY KEY,
                pair TEXT NOT NULL,
                entry_price REAL NOT NULL,
                exit_price REAL NOT NULL,
                pnl REAL NOT NULL,
                duration_seconds INTEGER NOT NULL,
                entry_time DATETIME NOT NULL,
                exit_time DATETIME NOT NULL,
                success BOOLEAN NOT NULL,
                market_regime TEXT NOT NULL,
                trend_strength REAL NOT NULL,
                volatility REAL NOT NULL,
                created_at DATETIME NOT NULL
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        // Create indexes for better performance
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

        sqlx::query("CREATE INDEX IF NOT EXISTS idx_price_feeds_source_pair ON price_feeds(source, pair)")
            .execute(&self.pool)
            .await?;

        sqlx::query("CREATE INDEX IF NOT EXISTS idx_technical_indicators_pair ON technical_indicators(pair)")
            .execute(&self.pool)
            .await?;

        sqlx::query("CREATE INDEX IF NOT EXISTS idx_technical_indicators_timestamp ON technical_indicators(timestamp)")
            .execute(&self.pool)
            .await?;

        sqlx::query("CREATE INDEX IF NOT EXISTS idx_trading_signals_pair ON trading_signals(pair)")
            .execute(&self.pool)
            .await?;

        sqlx::query("CREATE INDEX IF NOT EXISTS idx_trading_signals_timestamp ON trading_signals(timestamp)")
            .execute(&self.pool)
            .await?;

        sqlx::query("CREATE INDEX IF NOT EXISTS idx_trading_signals_executed ON trading_signals(executed)")
            .execute(&self.pool)
            .await?;

        sqlx::query("CREATE INDEX IF NOT EXISTS idx_positions_wallet_id ON positions(wallet_id)")
            .execute(&self.pool)
            .await?;

        sqlx::query("CREATE INDEX IF NOT EXISTS idx_positions_status ON positions(status)")
            .execute(&self.pool)
            .await?;

        sqlx::query("CREATE INDEX IF NOT EXISTS idx_positions_pair ON positions(pair)")
            .execute(&self.pool)
            .await?;

        sqlx::query("CREATE INDEX IF NOT EXISTS idx_trades_position_id ON trades(position_id)")
            .execute(&self.pool)
            .await?;

        sqlx::query("CREATE INDEX IF NOT EXISTS idx_trades_timestamp ON trades(timestamp)")
            .execute(&self.pool)
            .await?;

        sqlx::query("CREATE INDEX IF NOT EXISTS idx_performance_metrics_wallet_id ON performance_metrics(wallet_id)")
            .execute(&self.pool)
            .await?;

        sqlx::query("CREATE INDEX IF NOT EXISTS idx_performance_metrics_period ON performance_metrics(period)")
            .execute(&self.pool)
            .await?;

        info!("Enhanced database schema initialized successfully");
        Ok(())
    }

    pub async fn store_ml_trade_history(&self, trade: &MLTradeHistory) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO ml_trade_history (
                id, pair, entry_price, exit_price, pnl, duration_seconds,
                entry_time, exit_time, success, market_regime, trend_strength,
                volatility, created_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(&trade.id)
        .bind(&trade.pair)
        .bind(trade.entry_price)
        .bind(trade.exit_price)
        .bind(trade.pnl)
        .bind(trade.duration_seconds)
        .bind(&trade.entry_time)
        .bind(&trade.exit_time)
        .bind(trade.success)
        .bind(&trade.market_regime)
        .bind(trade.trend_strength)
        .bind(trade.volatility)
        .bind(&trade.created_at)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn get_ml_trade_history(&self, pair: &str, limit: Option<i32>) -> Result<Vec<MLTradeHistory>> {
        let limit = limit.unwrap_or(50);
        
        let rows = sqlx::query(
            r#"
            SELECT id, pair, entry_price, exit_price, pnl, duration_seconds,
                   entry_time, exit_time, success, market_regime, trend_strength,
                   volatility, created_at
            FROM ml_trade_history 
            WHERE pair = ?
            ORDER BY created_at DESC
            LIMIT ?
            "#,
        )
        .bind(pair)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        let trades = rows.into_iter().map(|row| {
            MLTradeHistory {
                id: row.try_get("id").unwrap_or_default(),
                pair: row.try_get("pair").unwrap_or_default(),
                entry_price: row.try_get("entry_price").unwrap_or_default(),
                exit_price: row.try_get("exit_price").unwrap_or_default(),
                pnl: row.try_get("pnl").unwrap_or_default(),
                duration_seconds: row.try_get("duration_seconds").unwrap_or_default(),
                entry_time: row.try_get("entry_time").unwrap_or_default(),
                exit_time: row.try_get("exit_time").unwrap_or_default(),
                success: row.try_get("success").unwrap_or_default(),
                market_regime: row.try_get("market_regime").unwrap_or_default(),
                trend_strength: row.try_get("trend_strength").unwrap_or_default(),
                volatility: row.try_get("volatility").unwrap_or_default(),
                created_at: row.try_get("created_at").unwrap_or_default(),
            }
        }).collect();

        Ok(trades)
    }

    pub async fn get_ml_trade_stats(&self, pair: &str) -> Result<MLTradeStats> {
        let row = sqlx::query(
            r#"
            SELECT 
                COUNT(*) as total_trades,
                SUM(CASE WHEN success THEN 1 ELSE 0 END) as winning_trades,
                AVG(pnl) as avg_pnl,
                AVG(CASE WHEN success THEN pnl ELSE NULL END) as avg_win,
                AVG(CASE WHEN NOT success THEN pnl ELSE NULL END) as avg_loss
            FROM ml_trade_history 
            WHERE pair = ?
            "#,
        )
        .bind(pair)
        .fetch_one(&self.pool)
        .await?;

        let total_trades: i64 = row.try_get("total_trades")?;
        let winning_trades: i64 = row.try_get("winning_trades")?;
        let win_rate = if total_trades > 0 { winning_trades as f64 / total_trades as f64 } else { 0.0 };
        let avg_pnl: f64 = row.try_get("avg_pnl")?;
        let avg_win: Option<f64> = row.try_get("avg_win").ok();
        let avg_loss: Option<f64> = row.try_get("avg_loss").ok();

        Ok(MLTradeStats {
            total_trades: total_trades as usize,
            win_rate,
            avg_pnl,
            avg_win: avg_win.unwrap_or(0.0),
            avg_loss: avg_loss.unwrap_or(0.0),
        })
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

    // Technical Indicators methods
    pub async fn store_technical_indicators(&self, pair: &str, indicators: &crate::models::StoreTechnicalIndicatorsRequest) -> Result<crate::models::TechnicalIndicators> {
        let id = Uuid::new_v4().to_string();
        let now = Utc::now();

        sqlx::query(
            r#"
            INSERT INTO technical_indicators (
                id, pair, timestamp, sma_20, sma_50, sma_200, rsi_14,
                price_change_24h, price_change_percent_24h, volatility_24h,
                current_price, created_at
            )
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(&id)
        .bind(pair)
        .bind(&now)
        .bind(indicators.sma_20)
        .bind(indicators.sma_50)
        .bind(indicators.sma_200)
        .bind(indicators.rsi_14)
        .bind(indicators.price_change_24h)
        .bind(indicators.price_change_percent_24h)
        .bind(indicators.volatility_24h)
        .bind(indicators.current_price)
        .bind(&now)
        .execute(&self.pool)
        .await?;

        Ok(crate::models::TechnicalIndicators {
            id,
            pair: pair.to_string(),
            timestamp: now,
            sma_20: indicators.sma_20,
            sma_50: indicators.sma_50,
            sma_200: indicators.sma_200,
            rsi_14: indicators.rsi_14,
            price_change_24h: indicators.price_change_24h,
            price_change_percent_24h: indicators.price_change_percent_24h,
            volatility_24h: indicators.volatility_24h,
            current_price: indicators.current_price,
            created_at: now,
        })
    }

    pub async fn get_latest_technical_indicators(&self, pair: &str) -> Result<Option<crate::models::TechnicalIndicators>> {
        let row = sqlx::query(
            r#"
            SELECT id, pair, timestamp, sma_20, sma_50, sma_200, rsi_14,
                   price_change_24h, price_change_percent_24h, volatility_24h,
                   current_price, created_at
            FROM technical_indicators
            WHERE pair = ?
            ORDER BY timestamp DESC
            LIMIT 1
            "#,
        )
        .bind(pair)
        .fetch_optional(&self.pool)
        .await?;

        match row {
            Some(row) => Ok(Some(crate::models::TechnicalIndicators {
                id: row.try_get("id")?,
                pair: row.try_get("pair")?,
                timestamp: row.try_get("timestamp")?,
                sma_20: row.try_get("sma_20")?,
                sma_50: row.try_get("sma_50")?,
                sma_200: row.try_get("sma_200")?,
                rsi_14: row.try_get("rsi_14")?,
                price_change_24h: row.try_get("price_change_24h")?,
                price_change_percent_24h: row.try_get("price_change_percent_24h")?,
                volatility_24h: row.try_get("volatility_24h")?,
                current_price: row.try_get("current_price")?,
                created_at: row.try_get("created_at")?,
            })),
            None => Ok(None),
        }
    }

    // Trading Signals methods
    pub async fn store_trading_signal(&self, signal: &crate::models::StoreTradingSignalRequest) -> Result<crate::models::TradingSignal> {
        let id = Uuid::new_v4().to_string();
        let now = Utc::now();

        sqlx::query(
            r#"
            INSERT INTO trading_signals (
                id, pair, signal_type, confidence, price, timestamp,
                reasoning, executed, created_at
            )
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(&id)
        .bind(&signal.pair)
        .bind(&signal.signal_type)
        .bind(signal.confidence)
        .bind(signal.price)
        .bind(&now)
        .bind(&signal.reasoning)
        .bind(false) // executed
        .bind(&now)
        .execute(&self.pool)
        .await?;

        Ok(crate::models::TradingSignal {
            id,
            pair: signal.pair.clone(),
            signal_type: signal.signal_type.clone(),
            confidence: signal.confidence,
            price: signal.price,
            timestamp: now,
            reasoning: signal.reasoning.clone(),
            executed: false,
            created_at: now,
        })
    }

    pub async fn get_trading_signals(&self, pair: &str, limit: i64) -> Result<Vec<crate::models::TradingSignal>> {
        let rows = sqlx::query(
            r#"
            SELECT id, pair, signal_type, confidence, price, timestamp,
                   reasoning, executed, created_at
            FROM trading_signals
            WHERE pair = ?
            ORDER BY timestamp DESC
            LIMIT ?
            "#,
        )
        .bind(pair)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        let signals: Vec<crate::models::TradingSignal> = rows
            .into_iter()
            .map(|row| crate::models::TradingSignal {
                id: row.try_get("id").unwrap_or_default(),
                pair: row.try_get("pair").unwrap_or_default(),
                signal_type: row.try_get("signal_type").unwrap_or_default(),
                confidence: row.try_get("confidence").unwrap_or_default(),
                price: row.try_get("price").unwrap_or_default(),
                timestamp: row.try_get("timestamp").unwrap_or_default(),
                reasoning: row.try_get("reasoning").ok(),
                executed: row.try_get("executed").unwrap_or_default(),
                created_at: row.try_get("created_at").unwrap_or_default(),
            })
            .collect();

        Ok(signals)
    }

    pub async fn get_recent_trading_signals(&self, limit: i64) -> Result<Vec<crate::models::TradingSignal>> {
        let rows = sqlx::query(
            r#"
            SELECT id, pair, signal_type, confidence, price, timestamp,
                   reasoning, executed, created_at
            FROM trading_signals
            ORDER BY timestamp DESC
            LIMIT ?
            "#,
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        let signals: Vec<crate::models::TradingSignal> = rows
            .into_iter()
            .map(|row| crate::models::TradingSignal {
                id: row.try_get("id").unwrap_or_default(),
                pair: row.try_get("pair").unwrap_or_default(),
                signal_type: row.try_get("signal_type").unwrap_or_default(),
                confidence: row.try_get("confidence").unwrap_or_default(),
                price: row.try_get("price").unwrap_or_default(),
                timestamp: row.try_get("timestamp").unwrap_or_default(),
                reasoning: row.try_get("reasoning").ok(),
                executed: row.try_get("executed").unwrap_or_default(),
                created_at: row.try_get("created_at").unwrap_or_default(),
            })
            .collect();

        Ok(signals)
    }

    // Positions methods
    pub async fn create_position(&self, request: &crate::models::CreatePositionRequest) -> Result<crate::models::Position> {
        let id = Uuid::new_v4().to_string();
        let now = Utc::now();

        // Get wallet by address
        let wallet = self.get_wallet_by_address(&request.wallet_address).await?;
        let wallet = wallet.ok_or_else(|| {
            DatabaseServiceError::NotFound("Wallet not found".to_string())
        })?;

        sqlx::query(
            r#"
            INSERT INTO positions (
                id, wallet_id, pair, position_type, entry_price, entry_time,
                quantity, status, created_at, updated_at
            )
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(&id)
        .bind(&wallet.id)
        .bind(&request.pair)
        .bind(&request.position_type)
        .bind(request.entry_price)
        .bind(&now)
        .bind(request.quantity)
        .bind("open")
        .bind(&now)
        .bind(&now)
        .execute(&self.pool)
        .await?;

        Ok(crate::models::Position {
            id,
            wallet_id: wallet.id,
            pair: request.pair.clone(),
            position_type: request.position_type.clone(),
            entry_price: request.entry_price,
            entry_time: now,
            quantity: request.quantity,
            status: "open".to_string(),
            exit_price: None,
            exit_time: None,
            pnl: None,
            pnl_percent: None,
            duration_seconds: None,
            created_at: now,
            updated_at: now,
            current_price: Some(request.entry_price),
        })
    }

    pub async fn close_position(&self, request: &crate::models::ClosePositionRequest) -> Result<crate::models::Position> {
        let now = Utc::now();
        
        info!("🔍 Looking up position: {}", request.position_id);

        // Get the position
        let position = match sqlx::query(
            r#"
            SELECT * FROM positions WHERE id = ?
            "#,
        )
        .bind(&request.position_id)
        .fetch_one(&self.pool)
        .await {
            Ok(pos) => pos,
            Err(sqlx::Error::RowNotFound) => {
                error!("❌ Position not found: {}", request.position_id);
                return Err(DatabaseServiceError::NotFound(format!("Position {} not found", request.position_id)));
            }
            Err(e) => {
                error!("❌ Database error looking up position {}: {}", request.position_id, e);
                return Err(DatabaseServiceError::Database(e));
            }
        };

        let entry_time: DateTime<Utc> = position.try_get("entry_time")?;
        let entry_price: f64 = position.try_get("entry_price")?;
        let quantity: f64 = position.try_get("quantity")?;
        let position_type: String = position.try_get("position_type")?;
        let status: String = position.try_get("status")?;
        
        // Check if position is already closed
        if status == "closed" {
            warn!("⚠️ Position {} is already closed", request.position_id);
            // Return the existing closed position
            return Ok(crate::models::Position {
                id: position.try_get("id")?,
                wallet_id: position.try_get("wallet_id")?,
                pair: position.try_get("pair")?,
                position_type: position.try_get("position_type")?,
                entry_price: position.try_get("entry_price")?,
                entry_time: position.try_get("entry_time")?,
                quantity: position.try_get("quantity")?,
                status: position.try_get("status")?,
                exit_price: position.try_get("exit_price")?,
                exit_time: position.try_get("exit_time")?,
                pnl: position.try_get("pnl")?,
                pnl_percent: position.try_get("pnl_percent")?,
                duration_seconds: position.try_get("duration_seconds")?,
                created_at: position.try_get("created_at")?,
                updated_at: position.try_get("updated_at")?,
                current_price: Some(position.try_get("exit_price").unwrap_or_else(|_| position.try_get("entry_price").unwrap_or(0.0))),
            });
        }

        info!("📊 Closing position: {} {} at ${:.4} -> ${:.4}", 
              position_type, request.position_id, entry_price, request.exit_price);

        // Calculate PnL
        let pnl = if position_type == "long" {
            (request.exit_price - entry_price) * quantity
        } else {
            (entry_price - request.exit_price) * quantity
        };

        // Calculate percentage based on price change (same as trading logic)
        let pnl_percent = if position_type == "long" {
            ((request.exit_price - entry_price) / entry_price) * 100.0
        } else {
            ((entry_price - request.exit_price) / entry_price) * 100.0
        };
        let duration_seconds = (now - entry_time).num_seconds();

        info!("💰 PnL: ${:.2} ({:.2}%) | Duration: {}s", pnl, pnl_percent, duration_seconds);

        // Update position status
        match sqlx::query(
            r#"
            UPDATE positions
            SET status = 'closed', exit_price = ?, exit_time = ?, pnl = ?, pnl_percent = ?,
                duration_seconds = ?, updated_at = ?
            WHERE id = ?
            "#,
        )
        .bind(request.exit_price)
        .bind(&now)
        .bind(pnl)
        .bind(pnl_percent)
        .bind(duration_seconds)
        .bind(&now)
        .bind(&request.position_id)
        .execute(&self.pool)
        .await {
            Ok(_) => info!("✅ Position status updated successfully"),
            Err(e) => {
                error!("❌ Failed to update position status: {}", e);
                return Err(DatabaseServiceError::Database(e));
            }
        }

        // Create trade record
        let trade_id = Uuid::new_v4().to_string();
        match sqlx::query(
            r#"
            INSERT INTO trades (
                id, position_id, trade_type, price, quantity, timestamp,
                transaction_hash, fees, created_at
            )
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(&trade_id)
        .bind(&request.position_id)
        .bind("sell")
        .bind(request.exit_price)
        .bind(quantity)
        .bind(&now)
        .bind(&request.transaction_hash)
        .bind(request.fees)
        .bind(&now)
        .execute(&self.pool)
        .await {
            Ok(_) => info!("✅ Trade record created successfully"),
            Err(e) => {
                error!("❌ Failed to create trade record: {}", e);
                return Err(DatabaseServiceError::Database(e));
            }
        }

        // Return updated position
        let updated_position = match sqlx::query(
            r#"
            SELECT * FROM positions WHERE id = ?
            "#,
        )
        .bind(&request.position_id)
        .fetch_one(&self.pool)
        .await {
            Ok(pos) => pos,
            Err(e) => {
                error!("❌ Failed to fetch updated position: {}", e);
                return Err(DatabaseServiceError::Database(e));
            }
        };

        info!("✅ Position {} closed successfully", request.position_id);

        Ok(crate::models::Position {
            id: updated_position.try_get("id")?,
            wallet_id: updated_position.try_get("wallet_id")?,
            pair: updated_position.try_get("pair")?,
            position_type: updated_position.try_get("position_type")?,
            entry_price: updated_position.try_get("entry_price")?,
            entry_time: updated_position.try_get("entry_time")?,
            quantity: updated_position.try_get("quantity")?,
            status: updated_position.try_get("status")?,
            exit_price: updated_position.try_get("exit_price")?,
            exit_time: updated_position.try_get("exit_time")?,
            pnl: updated_position.try_get("pnl")?,
            pnl_percent: updated_position.try_get("pnl_percent")?,
            duration_seconds: updated_position.try_get("duration_seconds")?,
            created_at: updated_position.try_get("created_at")?,
            updated_at: updated_position.try_get("updated_at")?,
            current_price: Some(updated_position.try_get("exit_price").unwrap_or_else(|_| updated_position.try_get("entry_price").unwrap_or(0.0))),
        })
    }

    pub async fn get_open_positions(&self, wallet_address: &str) -> Result<Vec<crate::models::Position>> {
        let rows = sqlx::query(
            r#"
            SELECT p.* FROM positions p
            JOIN wallets w ON p.wallet_id = w.id
            WHERE w.address = ? AND p.status = 'open'
            ORDER BY p.created_at DESC
            "#,
        )
        .bind(wallet_address)
        .fetch_all(&self.pool)
        .await?;

        let positions: Vec<crate::models::Position> = rows
            .into_iter()
            .map(|row| crate::models::Position {
                id: row.try_get("id").unwrap_or_default(),
                wallet_id: row.try_get("wallet_id").unwrap_or_default(),
                pair: row.try_get("pair").unwrap_or_default(),
                position_type: row.try_get("position_type").unwrap_or_default(),
                entry_price: row.try_get("entry_price").unwrap_or_default(),
                entry_time: row.try_get("entry_time").unwrap_or_default(),
                quantity: row.try_get("quantity").unwrap_or_default(),
                status: row.try_get("status").unwrap_or_default(),
                exit_price: row.try_get("exit_price").ok(),
                exit_time: row.try_get("exit_time").ok(),
                pnl: row.try_get("pnl").ok(),
                pnl_percent: row.try_get("pnl_percent").ok(),
                duration_seconds: row.try_get("duration_seconds").ok(),
                created_at: row.try_get("created_at").unwrap_or_default(),
                updated_at: row.try_get("updated_at").unwrap_or_default(),
                current_price: Some(row.try_get("entry_price").unwrap_or_default()),
            })
            .collect();

        Ok(positions)
    }

    pub async fn get_position_history(&self, wallet_address: &str, limit: i64) -> Result<Vec<crate::models::Position>> {
        let rows = sqlx::query(
            r#"
            SELECT p.* FROM positions p
            JOIN wallets w ON p.wallet_id = w.id
            WHERE w.address = ?
            ORDER BY p.created_at DESC
            LIMIT ?
            "#,
        )
        .bind(wallet_address)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        let positions: Vec<crate::models::Position> = rows
            .into_iter()
            .map(|row| crate::models::Position {
                id: row.try_get("id").unwrap_or_default(),
                wallet_id: row.try_get("wallet_id").unwrap_or_default(),
                pair: row.try_get("pair").unwrap_or_default(),
                position_type: row.try_get("position_type").unwrap_or_default(),
                entry_price: row.try_get("entry_price").unwrap_or_default(),
                entry_time: row.try_get("entry_time").unwrap_or_default(),
                quantity: row.try_get("quantity").unwrap_or_default(),
                status: row.try_get("status").unwrap_or_default(),
                exit_price: row.try_get("exit_price").ok(),
                exit_time: row.try_get("exit_time").ok(),
                pnl: row.try_get("pnl").ok(),
                pnl_percent: row.try_get("pnl_percent").ok(),
                duration_seconds: row.try_get("duration_seconds").ok(),
                created_at: row.try_get("created_at").unwrap_or_default(),
                updated_at: row.try_get("updated_at").unwrap_or_default(),
                current_price: Some(row.try_get("entry_price").unwrap_or_default()),
            })
            .collect();

        Ok(positions)
    }

    pub async fn get_all_positions(&self, limit: i64) -> Result<Vec<crate::models::Position>> {
        // Same query as query_trades.sh - get all positions, not filtered by wallet
        let rows = sqlx::query(
            r#"
            SELECT * FROM positions 
            ORDER BY entry_time DESC
            LIMIT ?
            "#,
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        let positions: Vec<crate::models::Position> = rows
            .into_iter()
            .map(|row| crate::models::Position {
                id: row.try_get("id").unwrap_or_default(),
                wallet_id: row.try_get("wallet_id").unwrap_or_default(),
                pair: row.try_get("pair").unwrap_or_default(),
                position_type: row.try_get("position_type").unwrap_or_default(),
                entry_price: row.try_get("entry_price").unwrap_or_default(),
                entry_time: row.try_get("entry_time").unwrap_or_default(),
                quantity: row.try_get("quantity").unwrap_or_default(),
                status: row.try_get("status").unwrap_or_default(),
                exit_price: row.try_get("exit_price").ok(),
                exit_time: row.try_get("exit_time").ok(),
                pnl: row.try_get("pnl").ok(),
                pnl_percent: row.try_get("pnl_percent").ok(),
                duration_seconds: row.try_get("duration_seconds").ok(),
                created_at: row.try_get("created_at").unwrap_or_default(),
                updated_at: row.try_get("updated_at").unwrap_or_default(),
                current_price: Some(row.try_get("entry_price").unwrap_or_default()),
            })
            .collect();

        Ok(positions)
    }

    // Trading Configs methods
    pub async fn create_trading_config(&self, request: &crate::models::CreateTradingConfigRequest) -> Result<crate::models::TradingConfig> {
        let id = Uuid::new_v4().to_string();
        let now = Utc::now();

        sqlx::query(
            r#"
            INSERT INTO trading_configs (
                id, name, pair, min_data_points, check_interval_secs,
                take_profit_percent, stop_loss_percent, max_position_size,
                enabled, created_at, updated_at
            )
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(&id)
        .bind(&request.name)
        .bind(&request.pair)
        .bind(request.min_data_points.unwrap_or(200))
        .bind(request.check_interval_secs.unwrap_or(30))
        .bind(request.take_profit_percent.unwrap_or(2.0))
        .bind(request.stop_loss_percent.unwrap_or(1.4))
        .bind(request.max_position_size.unwrap_or(100.0))
        .bind(true)
        .bind(&now)
        .bind(&now)
        .execute(&self.pool)
        .await?;

        Ok(crate::models::TradingConfig {
            id,
            name: request.name.clone(),
            pair: request.pair.clone(),
            min_data_points: request.min_data_points.unwrap_or(200),
            check_interval_secs: request.check_interval_secs.unwrap_or(30),
            take_profit_percent: request.take_profit_percent.unwrap_or(2.0),
            stop_loss_percent: request.stop_loss_percent.unwrap_or(1.4),
            max_position_size: request.max_position_size.unwrap_or(100.0),
            enabled: true,
            created_at: now,
            updated_at: now,
        })
    }

    pub async fn get_trading_config(&self, name: &str) -> Result<Option<crate::models::TradingConfig>> {
        let row = sqlx::query(
            r#"
            SELECT * FROM trading_configs WHERE name = ?
            "#,
        )
        .bind(name)
        .fetch_optional(&self.pool)
        .await?;

        match row {
            Some(row) => Ok(Some(crate::models::TradingConfig {
                id: row.try_get("id")?,
                name: row.try_get("name")?,
                pair: row.try_get("pair")?,
                min_data_points: row.try_get("min_data_points")?,
                check_interval_secs: row.try_get("check_interval_secs")?,
                take_profit_percent: row.try_get("take_profit_percent")?,
                stop_loss_percent: row.try_get("stop_loss_percent")?,
                max_position_size: row.try_get("max_position_size")?,
                enabled: row.try_get("enabled")?,
                created_at: row.try_get("created_at")?,
                updated_at: row.try_get("updated_at")?,
            })),
            None => Ok(None),
        }
    }

    pub async fn get_open_positions_by_pair(&self, pair: &str) -> Result<Option<crate::models::Position>> {
        let row = sqlx::query(
            r#"
            SELECT * FROM positions 
            WHERE pair = ? AND status = 'open'
            ORDER BY created_at DESC
            LIMIT 1
            "#,
        )
        .bind(pair)
        .fetch_optional(&self.pool)
        .await?;

        match row {
            Some(row) => Ok(Some(crate::models::Position {
                id: row.try_get("id")?,
                wallet_id: row.try_get("wallet_id")?,
                pair: row.try_get("pair")?,
                position_type: row.try_get("position_type")?,
                entry_price: row.try_get("entry_price")?,
                entry_time: row.try_get("entry_time")?,
                quantity: row.try_get("quantity")?,
                status: row.try_get("status")?,
                exit_price: row.try_get("exit_price").ok(),
                exit_time: row.try_get("exit_time").ok(),
                pnl: row.try_get("pnl").ok(),
                pnl_percent: row.try_get("pnl_percent").ok(),
                duration_seconds: row.try_get("duration_seconds").ok(),
                created_at: row.try_get("created_at")?,
                updated_at: row.try_get("updated_at")?,
                current_price: Some(row.try_get("entry_price")?),
            })),
            None => Ok(None),
        }
    }

    pub async fn update_position_status(&self, position_id: &str, status: &str) -> Result<crate::models::Position> {
        let now = Utc::now();
        
        sqlx::query(
            r#"
            UPDATE positions 
            SET status = ?, updated_at = ?
            WHERE id = ?
            "#,
        )
        .bind(status)
        .bind(&now)
        .bind(position_id)
        .execute(&self.pool)
        .await?;

        // Return updated position
        let row = sqlx::query(
            r#"
            SELECT * FROM positions WHERE id = ?
            "#,
        )
        .bind(position_id)
        .fetch_one(&self.pool)
        .await?;

        Ok(crate::models::Position {
            id: row.try_get("id")?,
            wallet_id: row.try_get("wallet_id")?,
            pair: row.try_get("pair")?,
            position_type: row.try_get("position_type")?,
            entry_price: row.try_get("entry_price")?,
            entry_time: row.try_get("entry_time")?,
            quantity: row.try_get("quantity")?,
            status: row.try_get("status")?,
            exit_price: row.try_get("exit_price").ok(),
            exit_time: row.try_get("exit_time").ok(),
            pnl: row.try_get("pnl").ok(),
            pnl_percent: row.try_get("pnl_percent").ok(),
            duration_seconds: row.try_get("duration_seconds").ok(),
            created_at: row.try_get("created_at")?,
            updated_at: row.try_get("updated_at")?,
            current_price: Some(row.try_get("entry_price")?),
        })
    }

    // Dashboard-specific methods
    pub async fn get_signals_count(&self, pair: &str, hours: i64) -> Result<i64> {
        let row = sqlx::query(
            r#"
            SELECT COUNT(*) as count FROM trading_signals 
            WHERE pair = ? AND timestamp >= datetime('now', '-' || ? || ' hours')
            "#,
        )
        .bind(pair)
        .bind(hours)
        .fetch_one(&self.pool)
        .await?;

        Ok(row.try_get("count")?)
    }

    pub async fn get_all_active_positions(&self) -> Result<Vec<crate::models::Position>> {
        let rows = sqlx::query(
            r#"
            SELECT * FROM positions 
            WHERE status = 'open'
            ORDER BY created_at DESC
            "#,
        )
        .fetch_all(&self.pool)
        .await?;

        let mut positions = Vec::new();
        
        for row in rows {
            let entry_price: f64 = row.try_get("entry_price").unwrap_or_default();
            let quantity: f64 = row.try_get("quantity").unwrap_or_default();
            let position_type: String = row.try_get("position_type").unwrap_or_default();
            let pair: String = row.try_get("pair").unwrap_or_default();
            let status: String = row.try_get("status").unwrap_or_default();
            let exit_price: Option<f64> = row.try_get("exit_price").ok();
            let pnl: Option<f64> = row.try_get("pnl").ok();
            let pnl_percent: Option<f64> = row.try_get("pnl_percent").ok();
            
            // For open positions, get the current market price to calculate unrealized PnL
            let current_price = if status == "open" {
                // Try to get the latest price from price feeds
                match self.get_latest_price(&pair, None).await {
                    Ok(Some(price_feed)) => price_feed.price,
                    _ => entry_price // Fallback to entry price if no current price available
                }
            } else {
                exit_price.unwrap_or(entry_price)
            };
            
            // Calculate unrealized PnL for open positions
            let (pnl_value, pnl_percent_value) = if status == "open" {
                // For open positions, calculate unrealized PnL
                let unrealized_pnl = if position_type == "long" {
                    (current_price - entry_price) * quantity
                } else {
                    (entry_price - current_price) * quantity
                };
                
                // Calculate percentage the same way as trading logic - based on price change
                let unrealized_pnl_percent = if entry_price > 0.0 {
                    if position_type == "long" {
                        ((current_price - entry_price) / entry_price) * 100.0
                    } else {
                        ((entry_price - current_price) / entry_price) * 100.0
                    }
                } else {
                    0.0
                };
                
                (unrealized_pnl, unrealized_pnl_percent)
            } else {
                // For closed positions, use stored values
                (pnl.unwrap_or(0.0), pnl_percent.unwrap_or(0.0))
            };
            
            positions.push(crate::models::Position {
                id: row.try_get("id").unwrap_or_default(),
                wallet_id: row.try_get("wallet_id").unwrap_or_default(),
                pair,
                position_type,
                entry_price,
                entry_time: row.try_get("entry_time").unwrap_or_default(),
                quantity,
                status,
                exit_price,
                exit_time: row.try_get("exit_time").ok(),
                pnl: Some(pnl_value), // Always provide a value for dashboard
                pnl_percent: Some(pnl_percent_value), // Always provide a value for dashboard
                duration_seconds: row.try_get("duration_seconds").ok(),
                created_at: row.try_get("created_at").unwrap_or_default(),
                updated_at: row.try_get("updated_at").unwrap_or_default(),
                current_price: Some(current_price), // Add current_price for dashboard compatibility
            });
        }

        Ok(positions)
    }

    pub async fn get_recent_trades(&self, limit: i64) -> Result<Vec<crate::models::Trade>> {
        // Get both entry and exit trades from all recent positions (open and closed)
        let rows = sqlx::query(
            r#"
            SELECT 
                id,
                pair,
                position_type,
                entry_price,
                exit_price,
                quantity,
                entry_time,
                exit_time,
                created_at,
                status,
                pnl,
                pnl_percent
            FROM positions 
            ORDER BY entry_time DESC
            LIMIT ?
            "#,
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        let mut trades: Vec<crate::models::Trade> = Vec::new();
        
        for row in rows {
            let position_id: String = row.try_get("id").unwrap_or_default();
            let pair: String = row.try_get("pair").unwrap_or_default();
            let quantity: f64 = row.try_get("quantity").unwrap_or_default();
            let entry_price: f64 = row.try_get("entry_price").unwrap_or_default();
            let exit_price: Option<f64> = row.try_get("exit_price").ok();
            let entry_time: DateTime<Utc> = row.try_get("entry_time").unwrap_or_default();
            let exit_time: Option<DateTime<Utc>> = row.try_get("exit_time").ok();
            let status: String = row.try_get("status").unwrap_or_default();
            let pnl: f64 = row.try_get("pnl").unwrap_or_default();
            let pnl_percent: f64 = row.try_get("pnl_percent").unwrap_or_default();
            
            // Create BUY trade (entry) - always exists
            let buy_trade = crate::models::Trade {
                id: format!("{}_buy", position_id),
                pair: pair.clone(),
                trade_type: "buy".to_string(),
                price: entry_price,
                quantity,
                total_value: quantity * entry_price,
                timestamp: entry_time,
                status: if status == "open" { "open".to_string() } else { "completed".to_string() },
                created_at: entry_time,
            };
            
            trades.push(buy_trade);
            
            // Create SELL trade (exit) - only for closed positions
            if let (Some(exit_price), Some(exit_time)) = (exit_price, exit_time) {
                let sell_trade = crate::models::Trade {
                    id: format!("{}_sell", position_id),
                    pair,
                    trade_type: "sell".to_string(),
                    price: exit_price,
                    quantity,
                    total_value: quantity * exit_price,
                    timestamp: exit_time,
                    status: "completed".to_string(),
                    created_at: exit_time,
                };
                trades.push(sell_trade);
            }
        }
        
        // Sort by timestamp (most recent first) and limit
        trades.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
        trades.truncate(limit as usize);

        Ok(trades)
    }

    pub async fn get_performance_metrics(&self) -> Result<serde_json::Value> {
        // Get total trades
        let total_trades_row = sqlx::query(
            r#"
            SELECT COUNT(*) as count FROM positions WHERE status = 'closed'
            "#,
        )
        .fetch_one(&self.pool)
        .await?;
        let total_trades: i64 = total_trades_row.try_get("count")?;

        // Get winning trades
        let winning_trades_row = sqlx::query(
            r#"
            SELECT COUNT(*) as count FROM positions 
            WHERE status = 'closed' AND pnl > 0
            "#,
        )
        .fetch_one(&self.pool)
        .await?;
        let winning_trades: i64 = winning_trades_row.try_get("count")?;

        // Get losing trades
        let losing_trades: i64 = total_trades - winning_trades;

        // Calculate win rate
        let win_rate = if total_trades > 0 {
            (winning_trades as f64 / total_trades as f64) * 100.0
        } else {
            0.0
        };

        // Get total PnL
        let total_pnl_row = sqlx::query(
            r#"
            SELECT COALESCE(SUM(pnl), 0.0) as total_pnl FROM positions 
            WHERE status = 'closed'
            "#,
        )
        .fetch_one(&self.pool)
        .await?;
        let total_pnl: f64 = total_pnl_row.try_get("total_pnl")?;

        // Get total PnL percent
        let total_pnl_percent_row = sqlx::query(
            r#"
            SELECT COALESCE(SUM(pnl_percent), 0.0) as total_pnl_percent FROM positions 
            WHERE status = 'closed'
            "#,
        )
        .fetch_one(&self.pool)
        .await?;
        let total_pnl_percent: f64 = total_pnl_percent_row.try_get("total_pnl_percent")?;

        // Calculate average trade PnL
        let avg_trade_pnl = if total_trades > 0 {
            total_pnl / total_trades as f64
        } else {
            0.0
        };

        // Get total volume
        let total_volume_row = sqlx::query(
            r#"
            SELECT COALESCE(SUM(quantity * entry_price), 0.0) as total_volume FROM positions
            "#,
        )
        .fetch_one(&self.pool)
        .await?;
        let total_volume: f64 = total_volume_row.try_get("total_volume")?;

        // Calculate max drawdown (simplified)
        let max_drawdown = 0.0; // TODO: Implement proper drawdown calculation

        // Calculate proper Sharpe ratio
        let sharpe_ratio = if total_trades > 0 {
            // Get all PnL values for closed positions
            let pnl_values: Vec<f64> = sqlx::query(
                r#"
                SELECT pnl FROM positions 
                WHERE status = 'closed' AND pnl IS NOT NULL
                ORDER BY exit_time ASC
                "#,
            )
            .fetch_all(&self.pool)
            .await?
            .into_iter()
            .filter_map(|row| row.try_get::<f64, _>("pnl").ok())
            .collect();

            if pnl_values.len() > 1 {
                // Calculate average return
                let avg_return = pnl_values.iter().sum::<f64>() / pnl_values.len() as f64;
                
                // Calculate standard deviation
                let variance = pnl_values.iter()
                    .map(|&x| (x - avg_return).powi(2))
                    .sum::<f64>() / pnl_values.len() as f64;
                let std_dev = variance.sqrt();
                
                // Risk-free rate (assume 0% for simplicity)
                let risk_free_rate = 0.0;
                
                // Sharpe ratio = (avg_return - risk_free_rate) / std_dev
                if std_dev > 0.0 {
                    (avg_return - risk_free_rate) / std_dev
                } else {
                    0.0
                }
            } else {
                0.0 // Need at least 2 trades for meaningful Sharpe ratio
            }
        } else {
            0.0
        };

        let metrics = serde_json::json!({
            "total_trades": total_trades,
            "winning_trades": winning_trades,
            "losing_trades": losing_trades,
            "win_rate": win_rate,
            "total_pnl": total_pnl,
            "total_pnl_percent": total_pnl_percent,
            "avg_trade_pnl": avg_trade_pnl,
            "max_drawdown": max_drawdown,
            "sharpe_ratio": sharpe_ratio,
            "total_volume": total_volume
        });

        Ok(metrics)
    }

    pub async fn store_candle(&self, pair: &str, interval: &str, open: f64, high: f64, low: f64, close: f64, volume: f64) -> Result<crate::models::Candle> {
        let id = Uuid::new_v4().to_string();
        let now = Utc::now();

        sqlx::query(
            r#"
            INSERT OR REPLACE INTO candles (id, pair, interval, open, high, low, close, volume, timestamp, created_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(&id)
        .bind(pair)
        .bind(interval)
        .bind(open)
        .bind(high)
        .bind(low)
        .bind(close)
        .bind(volume)
        .bind(now)
        .bind(now)
        .execute(&self.pool)
        .await?;

        Ok(crate::models::Candle {
            id,
            pair: pair.to_string(),
            interval: interval.to_string(),
            open,
            high,
            low,
            close,
            volume,
            timestamp: now,
            created_at: now,
        })
    }

    pub async fn get_candles(&self, pair: &str, interval: &str, limit: i64) -> Result<Vec<crate::models::Candle>> {
        let rows = sqlx::query(
            r#"
            SELECT id, pair, interval, open, high, low, close, volume, timestamp, created_at
            FROM candles 
            WHERE pair = ? AND interval = ?
            ORDER BY timestamp DESC
            LIMIT ?
            "#,
        )
        .bind(pair)
        .bind(interval)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        let candles: Vec<crate::models::Candle> = rows
            .into_iter()
            .map(|row| crate::models::Candle {
                id: row.try_get("id").unwrap_or_default(),
                pair: row.try_get("pair").unwrap_or_default(),
                interval: row.try_get("interval").unwrap_or_default(),
                open: row.try_get("open").unwrap_or_default(),
                high: row.try_get("high").unwrap_or_default(),
                low: row.try_get("low").unwrap_or_default(),
                close: row.try_get("close").unwrap_or_default(),
                volume: row.try_get("volume").unwrap_or_default(),
                timestamp: row.try_get("timestamp").unwrap_or_default(),
                created_at: row.try_get("created_at").unwrap_or_default(),
            })
            .collect();

        Ok(candles)
    }

    pub async fn get_latest_candle(&self, pair: &str, interval: &str) -> Result<Option<crate::models::Candle>> {
        let row = sqlx::query(
            r#"
            SELECT id, pair, interval, open, high, low, close, volume, timestamp, created_at
            FROM candles 
            WHERE pair = ? AND interval = ?
            ORDER BY timestamp DESC
            LIMIT 1
            "#,
        )
        .bind(pair)
        .bind(interval)
        .fetch_optional(&self.pool)
        .await?;

        match row {
            Some(row) => Ok(Some(crate::models::Candle {
                id: row.try_get("id").unwrap_or_default(),
                pair: row.try_get("pair").unwrap_or_default(),
                interval: row.try_get("interval").unwrap_or_default(),
                open: row.try_get("open").unwrap_or_default(),
                high: row.try_get("high").unwrap_or_default(),
                low: row.try_get("low").unwrap_or_default(),
                close: row.try_get("close").unwrap_or_default(),
                volume: row.try_get("volume").unwrap_or_default(),
                timestamp: row.try_get("timestamp").unwrap_or_default(),
                created_at: row.try_get("created_at").unwrap_or_default(),
            })),
            None => Ok(None),
        }
    }
} 