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

## 🚀 Quick Start Guide

### Prerequisites
- **Rust** (latest stable) - [Install Rust](https://rustup.rs/)
- **SQLite3** (optional, for database inspection)
- **Internet access** (for price feeds and Solana RPC)
- **Solana wallet** with USDC and SOL for trading

### Step 1: Clone and Setup
```bash
git clone <your-repo-url>
cd tirade
```

### Step 2: Configure Environment
```bash
# Copy the example environment file
cp env.example .env

# Edit the .env file with your settings
nano .env
```

**⚠️ IMPORTANT: The `.env` file must be in the project root directory:**
```
/home/travanx/projects/tirade/.env
```

**NOT in subdirectories like:**
```
/home/travanx/projects/tirade/trading-logic/.env  ❌
/home/travanx/projects/tirade/dashboard/.env      ❌
```

**Essential Configuration:**
```bash
# Your Solana private key (base58 encoded string)
SOLANA_PRIVATE_KEY=[your_private_key_here]

# Enable/disable real trading (false = paper trading only)
ENABLE_TRADING_EXECUTION=false

# Database configuration
DATABASE_URL=sqlite:../data/trading_bot.db
PRICE_FEED_DATABASE_URL=http://localhost:8080

# Trading parameters
MIN_CONFIDENCE_THRESHOLD=0.7
POSITION_SIZE_PERCENTAGE=0.5
SLIPPAGE_TOLERANCE=0.005
```

### Step 3: Initialize Database
```bash
# Create data directory and initialize SQLite database
mkdir -p data
touch data/trading_bot.db
```

### Step 4: Build All Services
```bash
# Build all services from the project root
cargo build

# Or build individual services:
cargo build --bin database-service
cargo build --bin price-feed
cargo build --bin trading-logic
cargo build --bin dashboard
```

### Step 5: Start Services (In Order)

**⚠️ IMPORTANT: Services must be started in this specific order for proper operation.**

#### Terminal 1: Database Service
```bash
# Start the database service first (required by all other services)
cd database-service
DATABASE_URL="sqlite:../data/trading_bot.db" cargo run
```

**Expected Output:**
```
🚀 Starting Database Service on http://0.0.0.0:8080
📊 Database URL: sqlite:../data/trading_bot.db
🌐 External Access: http://YOUR_VM_PUBLIC_IP:8080
```

#### Terminal 2: Price Feed
```bash
# Start the price feed service (feeds data to database)
cd price-feed
cargo run
```

**Expected Output:**
```
🚀 Starting Price Feed Service
📊 Database URL: http://localhost:8080
🔗 Pyth Feed: SOL/USDC
⏱️  Polling interval: 1 seconds
```

#### Terminal 3: Trading Logic
```bash
# Start the trading logic service (analyzes data and generates signals)
# The .env file should be in the project root directory (/home/travanx/projects/tirade/.env)
cd trading-logic
DATABASE_URL="http://localhost:8080" cargo run
```

**Expected Output:**
```
🚀 Starting Trading Logic Engine
📊 Database URL: http://localhost:8080
🔗 Trading Pair: SOL/USDC
⏱️  Check Interval: 30 seconds
🔄 Trading Execution: ENABLED (or PAPER TRADING)
```

#### Terminal 4: Dashboard (Optional)
```bash
# Start the web dashboard for monitoring
cd dashboard
DATABASE_URL="http://localhost:8080" cargo run
```

**Expected Output:**
```
🚀 Starting Tirade Dashboard on http://0.0.0.0:3000
📊 Database URL: http://localhost:8080
🌐 External Access: http://YOUR_VM_PUBLIC_IP:3000
```

### Step 6: Verify System Status

#### Check Database Service
```bash
curl http://localhost:8080/health
```
**Expected Response:** `{"status":"healthy"}`

#### Check Price Feed
- Look for price data being logged in Terminal 2
- Should see SOL/USDC prices every second

#### Check Trading Logic
- Look for trading analysis logs in Terminal 3
- Should see "Trading Analysis Report" every 30 seconds

#### Access Dashboard
- Open browser to: **http://localhost:3000**
- Should see real-time data and system status

### Step 7: Monitor and Configure

#### Dashboard Features
- **📊 Live Price Charts**: Real-time SOL/USDC price with technical indicators
- **�� Trading Signals**: Live signal generation with confidence levels
- **📈 Position Management**: Active positions and P&L tracking
- **🔧 System Status**: Service health indicators
- **⚡ Trading Execution**: Shows if real trading is enabled/disabled

#### Key Monitoring Points
1. **Database Connected**: Should show "Connected" (green)
2. **Price Feed Running**: Should show "Running" (green)
3. **Trading Logic Running**: Should show "Running" (green)
4. **Trading Execution**: Shows "Enabled" or "Disabled" based on your `.env` setting

### Step 8: Enable Real Trading (Optional)

**⚠️ WARNING: Only enable after thorough testing with paper trading!**

1. **Edit `.env` file:**
```bash
ENABLE_TRADING_EXECUTION=true
```

2. **Restart only the trading logic service:**
```bash
# In Terminal 3, stop with Ctrl+C, then restart:
cd trading-logic
DATABASE_URL="http://localhost:8080" cargo run
```

3. **Verify in dashboard:**
- Trading Execution should now show "Enabled" (green)

## 🔧 Service Management

### Stopping Services
```bash
# Stop individual services with Ctrl+C in their terminals
# Or kill all services:
pkill -f "cargo run"
```

### Restarting Services
```bash
# Always restart in order: Database → Price Feed → Trading Logic → Dashboard
# Only restart the specific service that needs updating
```

### Troubleshooting

#### Port Already in Use
```bash
# Check what's using the port
lsof -i:8080  # Database service
lsof -i:3000  # Dashboard

# Kill processes using the port
lsof -ti:8080 | xargs kill -9
lsof -ti:3000 | xargs kill -9
```

#### Database Connection Issues
```bash
# Verify database service is running
curl http://localhost:8080/health

# Check database file exists
ls -la data/trading_bot.db
```

#### Environment Variable Issues
```bash
# Verify .env file is in the correct location (project root)
ls -la .env

# Check environment variables are loaded
echo $DATABASE_URL
echo $ENABLE_TRADING_EXECUTION

# If trading logic can't find .env, verify it's in the project root:
# /home/travanx/projects/tirade/.env
# NOT in /home/travanx/projects/tirade/trading-logic/.env
```

## 📊 Expected System Behavior

### Normal Operation
- **Price Feed**: Updates every 1 second
- **Trading Logic**: Analyzes every 30 seconds
- **Dashboard**: Refreshes every 30 seconds
- **Database**: Stores all data persistently

### Log Messages to Watch For
```
✅ Database Service: "Starting Database Service"
✅ Price Feed: "Starting Price Feed Service"
✅ Trading Logic: "Starting Trading Logic Engine"
✅ Dashboard: "Starting Tirade Dashboard"
```

### Error Messages to Address
```
❌ "Address already in use" → Kill existing processes
❌ "Database connection failed" → Check database service is running
❌ "SOLANA_PRIVATE_KEY not found" → Check .env file is in project root directory
❌ "Permission denied" → Check file permissions and ownership
❌ "Looking for .env file at: /home/travanx/projects/tirade/../.env" → .env file is in wrong location
```

## 🎯 Next Steps

1. **Monitor the dashboard** for 10-15 minutes to ensure stable operation
2. **Review trading signals** and confidence levels
3. **Test with paper trading** before enabling real trading
4. **Adjust strategy parameters** in `.env` if needed:
   - `MIN_CONFIDENCE_THRESHOLD=0.25` (lower for more frequent signals)
   - `POSITION_SIZE_PERCENTAGE=0.5` (adjust risk per trade)
   - `SLIPPAGE_TOLERANCE=0.005` (adjust for market conditions)
5. **Enable real trading** only after thorough testing

## 📞 Support

If you encounter issues:
1. Check all services are running in the correct order
2. Verify environment variables are set correctly
3. Check the logs in each terminal for error messages
4. Ensure ports 8080 and 3000 are available
5. Verify internet connectivity for price feeds 