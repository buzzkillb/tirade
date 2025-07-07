# Tirade: Solana Quant Trading Bot Suite

## Overview
Tirade is a modular, Rust-based trading bot system for Solana, designed for research and rapid development of quant strategies using only price feeds. It consists of three main components:

- **solana-trading-bot**: (WIP) Handles wallet management, balance checking, and (future) transaction execution.
- **price-feed**: Fetches real-time price data from Pyth and Jupiter APIs and stores it in a local database.
- **database-service**: REST API service with SQLite backend for storing and querying price and balance data, as well as calculating technical indicators.
- **trading-logic**: Runs the trading strategy, analyzes price history and indicators, and (future) will execute swaps. Currently, it only simulates trading logic and logs signals/decisions.

## Features
- Modular, async Rust codebase
- Real-time price ingestion from Pyth and Jupiter
- REST API for price history and technical indicators (SMA, RSI, volatility, etc.)
- Quant-style, price-only trading strategy (momentum, mean reversion, volatility breakout)
- Configurable via `.env` files
- Database auto-initialization script

## Quick Start

### 1. Clone the repository
```bash
git clone <your-repo-url>
cd tirade
```

### 2. Set up environment variables
Copy the example env files and edit as needed:
```bash
cp price-feed/env.example price-feed/.env
cp trading-logic/env.example trading-logic/.env
```

### 3. Initialize the database
Run the provided script to create the data directory and SQLite file:
```bash
./init-database.sh
```

### 4. Start all services (in separate terminals or with `&`)
#### a. Start the database-service
```bash
cd database-service
DATABASE_URL="sqlite:../data/trading_bot.db" cargo run
```
#### b. Start the price-feed
```bash
cd price-feed
cargo run
```
#### c. Start the trading-logic
```bash
cd trading-logic
../target/debug/trading-logic
```

### 5. Monitor logs
Each service logs to the console. The trading-logic bot will show progress and analysis as data accumulates.

## API Endpoints (database-service)
- `GET /prices/{pair}/history?hours={hours}`: Price history
- `GET /prices/{pair}/latest?source={source}`: Latest price (by source)
- `GET /indicators/{pair}?hours={hours}`: Technical indicators (SMA, RSI, volatility, etc.)

## Notes
- **Swap execution is NOT yet implemented in trading-logic.** The bot currently only simulates trading logic and logs signals/decisions. Actual on-chain swap execution will be added in a future release.
- All configuration is via `.env` files in each service directory.
- The database is auto-initialized on first run, but you must run `./init-database.sh` to create the data directory.

## Requirements
- Rust (latest stable)
- SQLite3 (for inspecting the DB, optional)
- Internet access (for price feeds)

## Security
- **Do not commit private keys or sensitive information.**
- This project is for research and development only. Use at your own risk.

## License
MIT 