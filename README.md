# Tirade: Solana Quant Trading Bot Suite

## Overview
Tirade is a complete, production-ready Rust-based trading bot system for Solana, designed for automated quant trading with real-time execution capabilities. It consists of five main components:

- **solana-trading-bot**: Handles wallet management, balance checking, and actual transaction execution via Jupiter swaps.
- **price-feed**: Fetches real-time price data from Pyth and Jupiter APIs and stores it in a local database.
- **database-service**: REST API service with SQLite backend for storing and querying price data, technical indicators, trading signals, positions, and performance metrics.
- **trading-logic**: Runs the trading strategy, analyzes price history and indicators, generates signals, and executes trades with full persistence across restarts.
- **dashboard**: Real-time web dashboard for monitoring trading performance, system status, and market data.

## Features
- **Complete Trading Pipeline**: From price feeds to actual trade execution
- **Real-time Trade Execution**: Direct integration with Solana blockchain via Jupiter
- **Dry Run Mode**: Safe testing without actual trades
- **Balance Tracking**: Real-time wallet balance monitoring and position sizing
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
- **Real-time Web Dashboard** with interactive charts and metrics
- **Configurable via `.env` files**
- **Database auto-initialization** with enhanced schema

## Quick Start

### 1. Clone the repository
```bash
git clone <your-repo-url>
cd tirade
```

### 2. Set up environment variables
Copy the example env file and configure your settings:
```bash
cp env.example .env
```

Edit `.env` with your configuration:
```bash
# Solana Trading Bot Environment Variables

# Your Solana private key (base58 encoded string from solana-cli)
SOLANA_PRIVATE_KEY=your_private_key_here

# Solana RPC URL (optional - defaults to mainnet-beta if not set)
SOLANA_RPC_URL=https://api.mainnet-beta.solana.com

# --- Trading Execution Configuration ---
# Enable actual trading execution (false = paper trading only)
ENABLE_TRADING_EXECUTION=false

# Position sizing: percentage of USDC balance to use per trade (0.0 to 1.0)
POSITION_SIZE_PERCENTAGE=0.9

# Slippage tolerance: maximum acceptable slippage (0.0 to 1.0)
SLIPPAGE_TOLERANCE=0.005

# Minimum confidence threshold: minimum confidence to execute trades (0.0 to 1.0)
MIN_CONFIDENCE_THRESHOLD=0.7

# Maximum concurrent positions allowed
MAX_CONCURRENT_POSITIONS=1

# --- Database Configuration ---
DATABASE_URL=sqlite:../data/trading_bot.db
PRICE_FEED_DATABASE_URL=http://localhost:8080

# --- Trading Logic Configuration ---
TRADING_PAIR=SOL/USDC
MIN_DATA_POINTS=200
CHECK_INTERVAL_SECS=30
STOP_LOSS_THRESHOLD=0.02
TAKE_PROFIT_THRESHOLD=0.015
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
#### d. Start the dashboard (optional)
```bash
./start_dashboard.sh
```
Or manually:
```bash
cd dashboard
DATABASE_URL="http://localhost:8080" cargo run
```

### 5. Monitor logs and dashboard
Each service logs to the console. The trading-logic bot will show:
- Position recovery on startup
- Real-time trading analysis
- Signal generation and confidence levels
- Position management (open/close)
- Performance metrics
- Trade execution (dry run or real)

**🌐 Dashboard Access**: Open http://localhost:3000 in your browser to view the real-time trading dashboard with:
- Live price charts and technical indicators
- Trading signals and confidence levels
- Active positions and P&L tracking
- Performance metrics and statistics
- System status monitoring
- Auto-refreshing data every 30 seconds

## Trading Execution

### Dry Run Mode (Default)
- **Safe Testing**: No actual trades executed
- **Full Simulation**: Complete pipeline testing
- **Balance Calculation**: Uses real wallet balances for position sizing
- **No Database Impact**: Dry runs don't affect profit tracking

### Real Trading Mode
To enable real trading, set in `.env`:
```bash
ENABLE_TRADING_EXECUTION=true
```

**⚠️ WARNING**: Real trading will execute actual swaps on Solana mainnet using your wallet funds.

### Position Sizing
- **USDC Trades**: Uses 90% of available USDC balance (configurable)
- **SOL Trades**: Uses 90% of available SOL balance (configurable)
- **Single Position**: Maximum 1 concurrent position (configurable)
- **Slippage Protection**: 0.5% maximum slippage tolerance

### Balance Tracking
The system automatically:
- Checks wallet balances before each trade
- Calculates position sizes based on available funds
- Tracks performance and P&L
- Stores all trade data in the database

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

### Dashboard Endpoints
- `GET /signals/{pair}/count?hours={hours}`: Get signal count for dashboard
- `GET /positions/active`: Get all active positions for dashboard
- `GET /trades/recent`: Get recent trades for dashboard
- `GET /performance/metrics`: Get performance metrics for dashboard

### Health & Management
- `GET /health`: Service health check
- `POST /wallets`: Create wallet
- `POST /balances`: Store balance snapshot
- `GET /wallets/{address}/balances`: Get wallet balance history

## Real-time Dashboard

### Features
The Tirade dashboard provides a comprehensive web interface for monitoring your trading bot:

#### 📊 **Live Price Charts**
- Interactive 24-hour price charts with Chart.js
- Real-time price updates from Pyth and Jupiter
- Technical indicator overlays (SMA, RSI)

#### 🎯 **Trading Signals**
- Live signal generation with confidence levels
- Signal history and reasoning
- Color-coded signal types (Buy/Sell/Hold)

#### 📈 **Position Management**
- Active position tracking with real-time P&L
- Position entry/exit prices and timing
- Performance metrics and statistics

#### 💰 **Performance Analytics**
- Total trades, win rate, and P&L tracking
- Sharpe ratio and risk metrics
- Trading volume and activity statistics

#### 🔧 **System Monitoring**
- Service status indicators (Database, Price Feed, Trading Logic)
- Connection health and uptime
- Error tracking and alerts

#### 🎨 **Ultra-Dark Solana Theme**
- Beautiful ultra-dark theme inspired by Solana.com
- Solana purple (`#9945ff`) and mint green (`#14f195`) accents
- Pure black to dark gray gradients for eye comfort
- Professional styling with enhanced glow effects
- Responsive design with auto-refreshing data every 30 seconds
- Mobile-friendly interface optimized for extended viewing

### Access
- **URL**: http://localhost:3000
- **Auto-refresh**: Every 30 seconds
- **No authentication required** (local development)

### Technical Stack
- **Backend**: Rust with Actix-web
- **Frontend**: HTML5, CSS3, JavaScript with Chart.js
- **Charts**: Chart.js with date-fns adapter and Solana-themed styling
- **Data**: Real-time API calls to database service
- **Theme**: Ultra-dark Solana-inspired design with purple and green accents

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
- **Real Execution**: Direct integration with Solana blockchain

## Recent Improvements

### v3.1 Features (Current)
- ✅ **Ultra-Dark Solana Theme**: Beautiful dark theme inspired by Solana.com
- ✅ **Enhanced Visual Design**: Professional styling with purple and green accents
- ✅ **Complete Trading Execution**: Real Solana trades via Jupiter integration
- ✅ **Dry Run Mode**: Safe testing without actual trades
- ✅ **Balance Tracking**: Real-time wallet balance monitoring
- ✅ **Position Sizing**: Configurable position sizes (90% of balance)
- ✅ **Slippage Protection**: 0.5% maximum slippage tolerance
- ✅ **Single Position Management**: One trade at a time with configurable limits
- ✅ **Enhanced Environment Configuration**: Centralized .env configuration
- ✅ **Real-time Trade Execution**: Direct blockchain integration
- ✅ **Comprehensive Logging**: Detailed execution logs and performance tracking

### v2.0 Features
- ✅ **Enhanced Database Schema**: Comprehensive data storage for all trading activities
- ✅ **Position Persistence**: Trading logic recovers positions after restarts
- ✅ **Complete Data Storage**: All signals, indicators, positions, and trades are stored
- ✅ **Improved API**: RESTful endpoints for all data access
- ✅ **Better Error Handling**: Comprehensive error handling and logging
- ✅ **Dynamic Strategy**: Adaptive thresholds based on market conditions
- ✅ **Performance Tracking**: Built-in performance metrics and trade history

## Configuration

### Trading Parameters
- **Position Size**: 90% of available balance (configurable)
- **Slippage Tolerance**: 0.5% (50 basis points)
- **Confidence Threshold**: 70% minimum for trade execution
- **Max Positions**: 1 concurrent position
- **Check Interval**: 30 seconds between analyses

### Environment Variables
All configuration is centralized in the root `.env` file:
- Solana wallet configuration
- Trading execution settings
- Database connections
- Strategy parameters

## Notes
- **Dry runs are completely safe** and don't affect your database or profit tracking
- **Real trading requires careful configuration** - test thoroughly with dry runs first
- All configuration is via the root `.env` file
- The database is auto-initialized on first run with the enhanced schema
- Position state is automatically recovered when the trading logic restarts
- The system uses Jupiter for best execution and liquidity

## Requirements
- Rust (latest stable)
- SQLite3 (for inspecting the DB, optional)
- Internet access (for price feeds and Solana RPC)
- Solana wallet with USDC and SOL for trading

## Security
- **Do not commit private keys or sensitive information.**
- **Test thoroughly with dry runs before enabling real trading.**
- This project is for research and development. Use at your own risk.
- **Never share your private keys or .env file.**

## License
MIT 