# Tirade: Solana Quant Trading Bot Suite

## Overview
Tirade is a modular, Rust-based trading bot system for Solana, designed for research and rapid development of quant strategies using only price feeds. It consists of four main components:

- **solana-trading-bot**: (WIP) Handles wallet management, balance checking, and (future) transaction execution.
- **price-feed**: Fetches real-time price data from Pyth and Jupiter APIs and stores it in a local database.
- **database-service**: REST API service with SQLite backend for storing and querying price data, technical indicators, trading signals, positions, and performance metrics.
- **trading-logic**: Runs the trading strategy, analyzes price history and indicators, generates signals, and manages positions with full persistence across restarts.

## Features
- **Modular, async Rust codebase** with comprehensive error handling
- **Real-time price ingestion** from Pyth and Jupiter APIs
- **Enhanced database schema** with support for:
  - Price feeds and historical data
  - Technical indicators (SMA, RSI, volatility, etc.)
  - Trading signals with confidence levels
  - Position tracking and management
  - Trade history and performance metrics
  - Trading configurations
- **Position persistence** - trading logic recovers positions after restarts
- **Comprehensive data storage** - all signals, indicators, and trades are stored
- **Quant-style trading strategy** with multiple signal types:
  - RSI-based mean reversion
  - Trend-following with moving averages
  - Dynamic confidence thresholds
  - Take profit and stop loss management
- **REST API** for all data access and management
- **Configurable via `.env` files**
- **Database auto-initialization** with enhanced schema

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
cargo run
```

### 5. Monitor logs
Each service logs to the console. The trading-logic bot will show:
- Position recovery on startup
- Real-time trading analysis
- Signal generation and confidence levels
- Position management (open/close)
- Performance metrics

## Enhanced Database Schema

### Tables
- **price_feeds**: Real-time price data from multiple sources
- **technical_indicators**: Calculated indicators (SMA, RSI, volatility)
- **trading_signals**: Generated trading signals with confidence levels
- **positions**: Open and closed trading positions
- **trades**: Detailed trade history with PnL
- **trading_configs**: Strategy configurations
- **performance_metrics**: Portfolio performance tracking
- **wallets**: Wallet management (for future use)
- **balance_snapshots**: Balance history (for future use)

## API Endpoints (database-service)

### Price Data
- `GET /prices/{pair}/history?hours={hours}`: Price history
- `GET /prices/{pair}/latest?source={source}`: Latest price (by source)
- `POST /prices`: Store new price data

### Technical Indicators
- `GET /indicators/{pair}?hours={hours}`: Get calculated indicators
- `POST /indicators/{pair}/store`: Store technical indicators
- `GET /indicators/{pair}/latest`: Get latest indicators

### Trading Signals
- `POST /signals`: Store trading signal
- `GET /signals/{pair}?limit={limit}`: Get trading signals

### Positions
- `POST /positions`: Create new position
- `POST /positions/close`: Close position
- `GET /positions/{address}/open`: Get open positions by wallet
- `GET /positions/pair/{pair}/open`: Get open positions by trading pair
- `PATCH /positions/{position_id}/status`: Update position status
- `GET /positions/{address}/history?limit={limit}`: Get position history

### Trading Configs
- `POST /configs`: Create trading configuration
- `GET /configs/{name}`: Get trading configuration

### Health & Management
- `GET /health`: Service health check
- `POST /wallets`: Create wallet
- `POST /balances`: Store balance snapshot
- `GET /wallets/{address}/balances`: Get wallet balance history

## Trading Strategy

### Signal Generation
The trading logic implements a multi-strategy approach:

1. **RSI Mean Reversion**: 
   - Buy when RSI < 25 (oversold)
   - Sell when RSI > 75 (overbought)

2. **Trend Following**:
   - Buy when price > SMA(20) and RSI in bullish range
   - Sell when price < SMA(20) and RSI in bearish range

3. **Dynamic Thresholds**:
   - Confidence thresholds adjust based on market volatility
   - Take profit and stop loss levels are dynamic

### Position Management
- **Position Persistence**: Positions are stored in database and recovered on restart
- **Risk Management**: Dynamic take profit and stop loss levels
- **Trade Tracking**: All trades are logged with PnL calculations

## Recent Improvements

### v2.0 Features
- ✅ **Enhanced Database Schema**: Comprehensive data storage for all trading activities
- ✅ **Position Persistence**: Trading logic recovers positions after restarts
- ✅ **Complete Data Storage**: All signals, indicators, positions, and trades are stored
- ✅ **Improved API**: RESTful endpoints for all data access
- ✅ **Better Error Handling**: Comprehensive error handling and logging
- ✅ **Dynamic Strategy**: Adaptive thresholds based on market conditions
- ✅ **Performance Tracking**: Built-in performance metrics and trade history

## Notes
- **Swap execution is NOT yet implemented in trading-logic.** The bot currently simulates trading logic and logs signals/decisions. Actual on-chain swap execution will be added in a future release.
- All configuration is via `.env` files in each service directory.
- The database is auto-initialized on first run with the enhanced schema.
- Position state is automatically recovered when the trading logic restarts.

## Requirements
- Rust (latest stable)
- SQLite3 (for inspecting the DB, optional)
- Internet access (for price feeds)

## Security
- **Do not commit private keys or sensitive information.**
- This project is for research and development only. Use at your own risk.

## License
MIT 