# 🚀 TiRADE - Advanced Trading Bot Suite

A sophisticated, real-time cryptocurrency trading bot built in Rust with multi-timeframe analysis, technical indicators, and automated execution on the Solana blockchain.

![TiRADE Dashboard](https://img.shields.io/badge/Status-Active-brightgreen)
![Rust](https://img.shields.io/badge/Rust-1.70+-orange)
![License](https://img.shields.io/badge/License-MIT-blue)

## 📋 Table of Contents

- [Overview](#-overview)
- [Architecture](#-architecture)
- [Trading Engine Logic](#-trading-engine-logic)
- [Quick Start](#-quick-start)
- [Installation](#-installation)
- [Configuration](#-configuration)
- [Usage](#-usage)
- [Dashboard](#-dashboard)
- [Troubleshooting](#-troubleshooting)
- [Contributing](#-contributing)
- [License](#-license)

## 🎯 Overview

TiRADE is a comprehensive trading bot suite that combines real-time market analysis, technical indicators, and automated execution. Built with Rust for performance and reliability, it features:

- **Real-time Price Feeds**: Multiple sources (Pyth, Jupiter, Coinbase)
- **Advanced Technical Analysis**: RSI, SMA, volatility calculations
- **Multi-timeframe Strategy**: Short, medium, and long-term analysis
- **Automated Execution**: Direct Solana blockchain integration
- **Live Dashboard**: Real-time monitoring and control
- **Risk Management**: Dynamic stop-loss and take-profit

## 🏗️ Architecture

```
┌─────────────────┐    ┌─────────────────┐    ┌─────────────────┐
│   Price Feed    │    │  Trading Logic  │    │   Dashboard     │
│                 │    │                 │    │                 │
│ • Pyth Network  │───▶│ • RSI Analysis  │───▶│ • Live Charts   │
│ • Jupiter API   │    │ • SMA Crossover │    │ • Signal Display│
│ • Coinbase API  │    │ • Volatility    │    │ • Position Mgmt │
└─────────────────┘    └─────────────────┘    └─────────────────┘
         │                       │                       │
         ▼                       ▼                       ▼
┌─────────────────┐    ┌─────────────────┐    ┌─────────────────┐
│  Database       │    │  Transaction    │    │  System Status  │
│  Service        │    │  Executor       │    │  Monitoring     │
│                 │    │                 │    │                 │
│ • SQLite Store  │    │ • Solana Wallet│    │ • Service Health│
│ • API Endpoints │    │ • Jupiter Swap  │    │ • Performance   │
│ • Position Mgmt │    │ • Transaction   │    │ • Logs          │
└─────────────────┘    └─────────────────┘    └─────────────────┘
```

## 📊 Candle Aggregation System (NEW)

### Data Flow Architecture

```
1-Second Price Feeds
       ↓
   Raw Data Storage
       ↓
  Candle Aggregation
       ↓
   OHLC Candles (30s, 1m, 5m)
       ↓
  Technical Analysis
       ↓
   Trading Signals
```

### Candle Intervals

| Interval | Use Case | Data Points | Analysis Type |
|----------|----------|-------------|---------------|
| 30s | High-frequency momentum | 30 seconds | Quick momentum shifts |
| 1m | Primary trading analysis | 60 seconds | **Recommended for signals** |
| 5m | Trend confirmation | 300 seconds | Longer-term trends |

### Benefits of Candle-Based Analysis

1. **Noise Reduction**: Eliminates 1-second price spikes and micro-movements
2. **Industry Standard**: OHLC candles are the standard for technical analysis
3. **Better Indicators**: RSI, SMA, and volatility calculations work optimally
4. **Reliable Signals**: More consistent trading signals with fewer false positives
5. **Scalable**: Can easily add more intervals (15m, 1h, 4h) for different strategies

### API Endpoints

```bash
# Get candles for analysis
GET /candles/SOL%2FUSDC/1m?limit=200

# Get latest candle
GET /candles/SOL%2FUSDC/1m/latest

# Store candle (internal)
POST /candles
```

## 🧠 Trading Engine Logic

### Core Strategy Components

#### 1. **Candle-Based Analysis (NEW)**
```rust
// Data Aggregation System
1-second price feeds → 30s/1m/5m OHLC candles → Technical Analysis
```

**Candle Intervals:**
- **30-second candles**: High-frequency momentum analysis
- **1-minute candles**: Primary trading analysis (recommended)
- **5-minute candles**: Trend confirmation and longer-term signals

**Benefits:**
- **Reduced Noise**: Aggregated data eliminates 1-second price spikes
- **Industry Standard**: OHLC candles are the standard for technical analysis
- **Better Indicators**: RSI, SMA, and volatility calculations work optimally
- **Robust Analysis**: More reliable signals with fewer false positives

#### 2. **Multi-timeframe Analysis**
```rust
// Timeframe Configuration
short_timeframe: 30 minutes    // Recent momentum
medium_timeframe: 2 hours      // Intermediate trends  
long_timeframe: 6 hours        // Overall direction
```

#### 3. **Technical Indicators**

**RSI (Relative Strength Index)**
- **Fast RSI (7 periods)**: Short-term momentum
- **Slow RSI (21 periods)**: Long-term momentum
- **Divergence Detection**: Momentum shifts between timeframes

**Moving Averages**
- **SMA20**: Short-term trend confirmation
- **SMA50**: Medium-term trend direction (enhanced)
- **Crossover Analysis**: Price vs SMA relationships

**Volatility Analysis**
- **20-period volatility**: Market regime detection
- **Dynamic thresholds**: Adaptive to market conditions

#### 4. **Signal Generation Logic**

```rust
// Enhanced Confidence Calculation
confidence = (
    rsi_divergence_score * 0.25 +
    ma_crossover_score * 0.25 +
    volatility_score * 0.20 +
    momentum_score * 0.20 +
    trend_confirmation * 0.10
);

// Signal Types with Position Enforcement
if confidence > 0.45 && no_current_position {
    SignalType::Buy
} else if current_position_exists && exit_conditions_met {
    SignalType::Sell
} else {
    SignalType::Hold
}
```

#### 5. **Risk Management**

**Dynamic Position Sizing**
```rust
position_size = wallet_balance * position_size_percentage
max_position = min(position_size, max_position_size)
```

**Stop Loss & Take Profit**
```rust
// Dynamic based on volatility
stop_loss = entry_price * (1 - volatility_multiplier)
take_profit = entry_price * (1 + volatility_multiplier * 1.67)
```

#### 6. **Position Management**

**Single Position Strategy**
- Only one position at a time
- Strict position enforcement with double safety checks
- 5-minute signal cooldown between trades
- Automatic position recovery on restart

**Position Lifecycle**
```
1. Signal Generation → 2. Position Creation → 3. Monitoring → 4. Exit
```

### Strategy Triggers

#### **Buy Signal Conditions**
1. **RSI Divergence**: Fast RSI > Slow RSI with momentum
2. **MA Crossover**: Price above SMA20 and SMA50
3. **Volatility Breakout**: Significant price movement
4. **Momentum Confirmation**: Trend strength validation
5. **No Current Position**: Single position enforcement
6. **Candle Confirmation**: 1-minute candle analysis

#### **Sell Signal Conditions**
1. **Take Profit Hit**: Dynamic profit target reached
2. **Stop Loss Hit**: Dynamic loss limit reached
3. **Trend Reversal**: Technical indicators show reversal
4. **Time-based Exit**: Maximum position duration

### Enhanced Error Handling (NEW)

**Robust HTTP Client:**
- Raw response logging for debugging
- Proper URL encoding for trading pairs
- Graceful handling of empty responses
- Manual JSON parsing to prevent EOF errors

**Database Communication:**
- Automatic retry logic for failed requests
- Detailed error logging for troubleshooting
- Fallback mechanisms for service outages

## 🚀 Quick Start

### Prerequisites
- **Rust 1.70+**: `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh`
- **Solana CLI**: `sh -c "$(curl -sSfL https://release.solana.com/v1.17.0/install)"`
- **Git**: `sudo apt install git`

### Installation

```bash
# 1. Clone the repository
git clone https://github.com/buzzkillb/tirade.git
cd tirade

# 2. Initialize the database
./init-database.sh

# 3. Configure environment
cp env.example .env
# Edit .env with your Solana wallet and API keys

# 4. Start all services
./start_all_screen.sh
```

### Access Dashboard
- **Local**: http://localhost:3000
- **Remote**: SSH tunnel: `ssh -L 3000:127.0.0.1:3000 user@your-vps`

## ⚙️ Installation

### Step 1: System Requirements

```bash
# Update system
sudo apt update && sudo apt upgrade -y

# Install dependencies
sudo apt install -y build-essential pkg-config libssl-dev curl git screen

# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source ~/.cargo/env

# Install Solana CLI
sh -c "$(curl -sSfL https://release.solana.com/v1.17.0/install)"
export PATH="$HOME/.local/share/solana/install/active_release/bin:$PATH"
```

### Step 2: Clone and Setup

```bash
# Clone repository
git clone https://github.com/buzzkillb/tirade.git
cd tirade

# Initialize database
./init-database.sh

# Build all components
cargo build --release
```

### Step 3: Configuration

```bash
# Copy environment template
cp env.example .env

# Edit configuration
nano .env
```

**Required Environment Variables:**
```bash
# Solana Configuration
SOLANA_PRIVATE_KEY="your_base58_private_key"
SOLANA_RPC_URL="https://api.mainnet-beta.solana.com"

# Trading Configuration
TRADING_PAIR="SOL/USDC"
POSITION_SIZE_PERCENTAGE=0.5
MIN_CONFIDENCE_THRESHOLD=0.45
SIGNAL_COOLDOWN_SECONDS=300

# Database Configuration
DATABASE_URL="sqlite:data/trading_bot.db"

# API Keys (Optional)
COINBASE_API_KEY="your_coinbase_key"
```

## 🎮 Usage

### Starting the Bot

```bash
# Start all services in screen sessions
./start_all_screen.sh

# Or start individually:
screen -S database -d -m bash -c "cd database-service && cargo run"
screen -S price-feed -d -m bash -c "cd price-feed && cargo run"
screen -S trading-logic -d -m bash -c "cd trading-logic && cargo run"
screen -S dashboard -d -m bash -c "cd dashboard && cargo run"
```

### Screen Management

```bash
# List all sessions
screen -ls

# Attach to specific session
screen -r database
screen -r trading-logic
screen -r dashboard

# Detach from session
# Press Ctrl+A, then D

# Kill specific session
screen -S database -X quit

# Kill all sessions
pkill screen
```

### Monitoring

```bash
# Check service status
./check_trading_status.sh

# View trading logs
tail -f logs/trading_logic.log

# Clear trading data
./clear_trading_data.sh

# Query positions
./query_trades.sh
```

## 📊 Dashboard

### Features

- **Real-time Price Charts**: Live SOL/USDC with technical indicators
- **Trading Signals**: Live signals with confidence levels and reasoning
- **Position Management**: Active positions with P&L tracking
- **System Status**: Service health and performance metrics
- **Technical Indicators**: RSI, SMA, volatility displays
- **Performance Metrics**: Win rate, total P&L, Sharpe ratio

### Dashboard Sections

#### **System Status**
- Database connectivity
- Price feed status
- Trading logic status
- Active position (Yes/No)
- Signal count today

#### **Live Exchange Prices**
- Pyth Network (🔮)
- Jupiter (🪐)
- Coinbase (🟢 LIVE)

#### **Trading Signals**
- Signal type (Buy/Sell/Hold)
- Confidence percentage
- Technical reasoning
- Strategy triggers

#### **Active Positions**
- Entry price and time
- Current P&L
- Position duration
- Exit conditions

## 🔧 Configuration

### Trading Parameters

```bash
# Strategy Configuration
MIN_DATA_POINTS=60              # Minimum data points before analysis
CHECK_INTERVAL_SECS=30          # Analysis frequency
CONFIDENCE_THRESHOLD=0.45       # Minimum confidence for trades
SIGNAL_COOLDOWN_SECS=300        # Cooldown between signals

# Risk Management
POSITION_SIZE_PERCENTAGE=0.5    # % of wallet per trade
MAX_POSITION_SIZE=100.0         # Maximum position size
TAKE_PROFIT_PERCENT=2.0         # Dynamic take profit
STOP_LOSS_PERCENT=1.4           # Dynamic stop loss

# Candle Aggregation (NEW)
PYTH_INTERVAL_SECS=1            # 1-second price collection
JUP_INTERVAL_SECS=10            # 10-second Jupiter updates
CANDLE_AGGREGATION_ENABLED=true # Enable OHLC candle creation
```

### Technical Indicators

```bash
# RSI Configuration
RSI_FAST_PERIOD=7              # Fast RSI periods
RSI_SLOW_PERIOD=21             # Slow RSI periods
RSI_OVERSOLD=30                # Oversold threshold
RSI_OVERBOUGHT=70              # Overbought threshold

# Moving Averages
SMA_SHORT_PERIOD=20            # Short-term SMA
SMA_LONG_PERIOD=50             # Long-term SMA (enhanced)

# Volatility
VOLATILITY_WINDOW=20           # Volatility calculation window

# Candle Analysis (NEW)
CANDLE_INTERVALS="30s,1m,5m"   # Available candle intervals
PRIMARY_CANDLE_INTERVAL="1m"    # Primary analysis interval
```

## 🛠️ Troubleshooting

### Common Issues

#### **Service Won't Start**
```bash
# Check if ports are in use
lsof -ti:8080 | xargs kill -9
lsof -ti:3000 | xargs kill -9

# Check environment variables
echo $SOLANA_PRIVATE_KEY
echo $DATABASE_URL
```

#### **Database Errors**
```bash
# Reinitialize database
./init-database.sh

# Check database file
ls -la data/trading_bot.db
```

#### **Trading Logic Issues**
```bash
# Check logs with enhanced debugging
tail -f logs/trading_logic.log

# Verify wallet
solana balance

# Test transaction binary
./solana-trading-bot/target/debug/transaction

# Check candle aggregation (NEW)
curl http://localhost:8080/candles/SOL%2FUSDC/1m?limit=10

# Debug HTTP responses (NEW)
# Look for "Raw candle response:" and "Raw price response:" in logs
```

#### **Candle Aggregation Issues (NEW)**
```bash
# Check if candles are being created
curl http://localhost:8080/candles/SOL%2FUSDC/1m/latest

# Verify price feed is running
curl http://localhost:8080/prices/SOL%2FUSDC

# Check candle aggregation logs
# Look for "Created X candle for SOL/USDC" in price-feed logs
```

#### **HTTP Client Errors (NEW)**
```bash
# Check for EOF errors (should be resolved)
grep "EOF while parsing" logs/trading_logic.log

# Check for URL encoding issues
grep "Raw.*response" logs/trading_logic.log

# Verify database service is responding
curl -v http://localhost:8080/health
```

#### **Dashboard Not Loading**
```bash
# Check dashboard service
curl http://localhost:3000

# Check database API
curl http://localhost:8080/health

# Restart dashboard
screen -S dashboard -X quit
cd dashboard && cargo run
```

### Error Messages

| Error | Solution |
|-------|----------|
| `SOLANA_PRIVATE_KEY not found` | Check .env file location and format |
| `Address already in use` | Kill existing processes on ports 8080/3000 |
| `Database connection failed` | Start database service first |
| `Permission denied` | Check file permissions and ownership |

### Performance Optimization

```bash
# Increase system limits
echo 'fs.file-max = 65536' | sudo tee -a /etc/sysctl.conf
sudo sysctl -p

# Optimize SQLite
echo 'PRAGMA journal_mode=WAL;' | sqlite3 data/trading_bot.db
```

## 📈 Performance Metrics

### Expected Behavior

- **Price Updates**: Every 1 second (raw data collection)
- **Candle Creation**: Every 30 seconds (30s, 1m, 5m intervals)
- **Trading Analysis**: Every 30 seconds (using 1-minute candles)
- **Dashboard Refresh**: Every 2 seconds
- **Signal Generation**: Based on confidence thresholds
- **Position Duration**: 5 minutes to several hours

### Success Indicators

- ✅ All services show "Running" status
- ✅ Dashboard displays real-time data
- ✅ Trading signals appear with confidence levels
- ✅ Database stores historical data and candles
- ✅ No EOF errors in trading-logic logs
- ✅ Candle aggregation working (check price-feed logs)
- ✅ Raw response logging shows valid JSON responses

## 🔒 Security

### Best Practices

1. **Environment Variables**: Never commit private keys
2. **SSH Access**: Use SSH keys, not passwords
3. **Firewall**: Only open necessary ports
4. **Regular Updates**: Keep system and dependencies updated
5. **Backup**: Regular database backups

### Paper Trading

```bash
# Enable paper trading mode
ENABLE_TRADING_EXECUTION=false

# Test with small amounts first
POSITION_SIZE_PERCENTAGE=0.1
```

## 🤝 Contributing

1. Fork the repository
2. Create a feature branch: `git checkout -b feature-name`
3. Make your changes
4. Test thoroughly
5. Submit a pull request

### Development Setup

```bash
# Install development dependencies
cargo install cargo-watch

# Run with hot reload
cargo watch -x run

# Run tests
cargo test

# Check formatting
cargo fmt
```

## 📄 License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## ⚠️ Disclaimer

**This software is for educational and research purposes only. Trading cryptocurrencies involves substantial risk of loss. Use at your own risk and never invest more than you can afford to lose.**

- Past performance does not guarantee future results
- Always test with paper trading first
- Monitor the bot continuously
- Understand the risks involved

## 📞 Support

For issues, questions, or contributions:

1. Check the [Troubleshooting](#-troubleshooting) section
2. Review service logs for error messages
3. Open an issue on GitHub
4. Join our community discussions

---

**Built with ❤️ in Rust for the Solana ecosystem** 