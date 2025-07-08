# Tirade: Advanced Solana Quant Trading Bot Suite

## 🚀 Overview
Tirade is a sophisticated, production-ready Rust-based trading bot system for Solana, featuring advanced multi-timeframe analysis, enhanced trading strategies, and real-time execution capabilities. Built with modern Rust async patterns and comprehensive error handling.

## 🎯 Key Features

### 🔥 **Enhanced Trading Logic**
- **Multi-timeframe Analysis**: Short-term, medium-term, and long-term strategy evaluation
- **7 Advanced Trading Strategies**:
  - RSI Divergence Detection
  - Moving Average Crossover
  - Volatility Breakout
  - Mean Reversion
  - RSI Threshold Analysis
  - Momentum Confirmation
  - Enhanced Trend Following
- **Dynamic Threshold Calculation**: Adaptive RSI and volatility thresholds based on market regime
- **Sophisticated Confidence Scoring**: Multi-strategy validation with 70%+ threshold for execution
- **Risk Management**: Dynamic take profit and stop loss calculations

### 📊 **Real-time Data & Analysis**
- **24-hour Historical Data**: Comprehensive price analysis with 5,000+ data points
- **Technical Indicators**: RSI (14), SMA (20), volatility analysis, trend strength
- **Market Sentiment Analysis**: Bullish/bearish signal counting and regime detection
- **High-frequency Price Feeds**: Real-time data from Pyth and Jupiter APIs

### 🖥️ **Enhanced Dashboard**
- **Real-time Signal Display**: Live trading signals with confidence levels
- **Strategy Trigger Visualization**: Checkboxes showing active trading strategies
- **Detailed Analysis**: Comprehensive reasoning and signal details
- **Risk Management Display**: Take profit and stop loss levels for each signal
- **Multi-timeframe Charts**: Visual representation of short/medium/long-term analysis

### 🏗️ **System Architecture**
- **Modular Design**: 5 independent services with clear separation of concerns
- **Database Service**: REST API with SQLite backend for data persistence
- **Price Feed**: Real-time market data ingestion
- **Trading Logic**: Advanced strategy execution and signal generation
- **Dashboard**: Real-time web interface for monitoring
- **Trading Bot**: Solana blockchain integration via Jupiter

## 🛠️ Installation & Setup

### Prerequisites
- **Rust** (latest stable) - [Install Rust](https://rustup.rs/)
- **SQLite3** (for database inspection)
- **Internet access** (for price feeds and Solana RPC)
- **Solana wallet** with USDC and SOL for trading

### Quick Start

#### 1. Clone Repository
```bash
git clone https://github.com/yourusername/tirade.git
cd tirade
```

#### 2. Configure Environment
```bash
# Copy your .env file to project root
cp /path/to/your/.env ./

# Or create new .env file
nano .env
```

**⚠️ CRITICAL: .env file must be in project root directory:**
```
/home/user/tirade/.env  ✅
/home/user/tirade/trading-logic/.env  ❌
```

**Essential Configuration:**
```bash
# Solana Configuration
SOLANA_PRIVATE_KEY=[your_private_key_here]
ENABLE_TRADING_EXECUTION=false  # Set to true for real trading

# Database Configuration
DATABASE_URL=sqlite:../data/trading_bot.db
PRICE_FEED_DATABASE_URL=http://localhost:8080

# Trading Parameters
MIN_CONFIDENCE_THRESHOLD=0.7
POSITION_SIZE_PERCENTAGE=0.5
SLIPPAGE_TOLERANCE=0.005
```

#### 3. Automated Startup (Recommended)
```bash
# Make scripts executable
chmod +x start-tirade.sh init-database.sh

# Start all services with one command
./start-tirade.sh
```

**The startup script will:**
- ✅ Check environment and .env file
- 🗄️ Initialize database automatically
- 📊 Start Database Service (Port 8080)
- 📈 Start Price Feed (Port 8081)
- 🧠 Start Trading Logic
- 🌐 Start Dashboard (localhost:3000)

#### 4. Manual Startup (Alternative)
```bash
# Terminal 1: Database Service
DATABASE_URL="http://localhost:8080" cargo run --bin database-service

# Terminal 2: Price Feed
DATABASE_URL="http://localhost:8080" cargo run --bin price-feed

# Terminal 3: Trading Logic
DATABASE_URL="http://localhost:8080" cargo run --bin trading-logic

# Terminal 4: Dashboard
DATABASE_URL="http://localhost:8080" cargo run --bin dashboard
```

## 🌐 Dashboard Access

### Local Access
- **URL**: http://localhost:3000
- **Security**: Binds to localhost only (127.0.0.1)

### Remote Access (VPS Deployment)
```bash
# SSH Tunnel (Recommended)
ssh -L 3000:127.0.0.1:3000 user@vps-ip
# Then access http://localhost:3000 from your local machine

# Or modify startup script for external access:
# Change --host 127.0.0.1 to --host 0.0.0.0 in start-tirade.sh
```

## 📊 Trading Strategy Details

### Multi-timeframe Analysis
- **Short-term**: Recent price movements and momentum
- **Medium-term**: Intermediate trend analysis
- **Long-term**: Overall market direction and structure

### Strategy Components
1. **RSI Divergence**: Detects momentum shifts between fast and slow RSI
2. **Moving Average Crossover**: Price vs SMA analysis with trend confirmation
3. **Volatility Breakout**: Identifies significant price movements
4. **Mean Reversion**: Extreme RSI conditions (oversold/overbought)
5. **RSI Threshold**: Dynamic oversold/overbought levels based on market regime
6. **Momentum Confirmation**: Validates trend strength and direction
7. **Enhanced Trend Following**: Multi-factor trend analysis with regime detection

### Risk Management
- **Dynamic Take Profit**: 1.60% based on volatility and market conditions
- **Dynamic Stop Loss**: 0.96% to limit downside risk
- **Position Sizing**: Configurable percentage of wallet balance
- **Confidence Threshold**: 70% minimum for trade execution

## 🚀 VPS Deployment

### Option 1: Git Clone (Recommended)
```bash
# On VPS
git clone https://github.com/yourusername/tirade.git
cd tirade

# Copy your .env file
scp /path/to/your/.env user@vps-ip:/home/user/tirade/

# Install dependencies
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source ~/.cargo/env
sudo apt update && sudo apt install -y build-essential pkg-config libssl-dev

# Start everything
./start-tirade.sh
```

### Option 2: Archive Transfer
```bash
# On your local machine
tar -czf tirade-backup.tar.gz tirade/
scp tirade-backup.tar.gz user@vps-ip:/home/user/

# On VPS
tar -xzf tirade-backup.tar.gz
cd tirade
./start-tirade.sh
```

### VPS Configuration
```bash
# Open required ports
sudo ufw allow 3000  # Dashboard
sudo ufw allow 8080  # Database API

# For external dashboard access, modify start-tirade.sh:
# Change --host 127.0.0.1 to --host 0.0.0.0
```

## 📈 System Monitoring

### Dashboard Features
- **📊 Live Price Charts**: Real-time SOL/USDC with technical indicators
- **🎯 Trading Signals**: Live signals with confidence levels and strategy triggers
- **📈 Position Management**: Active positions and P&L tracking
- **🔧 System Status**: Service health indicators
- **⚡ Trading Execution**: Real-time trading status
- **📊 Signal Analysis**: Detailed reasoning and strategy breakdown

### Log Monitoring
```bash
# View all service logs
tail -f /var/log/syslog | grep -E '(trading|dashboard|price)'

# Check specific service
ps aux | grep cargo
```

### Health Checks
```bash
# Database service
curl http://localhost:8080/health

# Dashboard
curl http://localhost:3000
```

## 🔧 Configuration & Tuning

### Trading Parameters
```bash
# .env file adjustments
MIN_CONFIDENCE_THRESHOLD=0.7    # Higher = fewer, more confident trades
POSITION_SIZE_PERCENTAGE=0.5    # % of wallet per trade
SLIPPAGE_TOLERANCE=0.005        # Maximum acceptable slippage
```

### Strategy Tuning
The system automatically adjusts based on market conditions:
- **Trending Markets**: Higher confidence thresholds, longer holding periods
- **Consolidating Markets**: Lower thresholds, shorter holding periods
- **High Volatility**: Wider stop losses, higher take profits
- **Low Volatility**: Tighter stops, lower take profits

## 🛡️ Security Features

### Dashboard Security
- **Localhost Binding**: Dashboard binds to 127.0.0.1 by default
- **SSH Tunnel Access**: Secure remote access via SSH tunneling
- **No External Exposure**: Prevents unauthorized access

### Trading Security
- **Paper Trading Mode**: Safe testing without real funds
- **Confidence Thresholds**: Prevents low-quality trades
- **Position Limits**: Configurable risk per trade
- **Stop Loss Protection**: Automatic loss limiting

## 🚨 Troubleshooting

### Common Issues
```bash
# Port already in use
lsof -ti:8080 | xargs kill -9
lsof -ti:3000 | xargs kill -9

# Database not found
./init-database.sh

# Environment variables not loaded
# Ensure .env is in project root directory

# Services not starting
# Check all services are running in correct order
```

### Error Messages
- **"SOLANA_PRIVATE_KEY not found"**: Check .env file location
- **"Address already in use"**: Kill existing processes
- **"Database connection failed"**: Start database service first
- **"Permission denied"**: Check file permissions

## 📊 Performance Metrics

### Expected Behavior
- **Price Updates**: Every 1 second
- **Trading Analysis**: Every 30 seconds
- **Dashboard Refresh**: Every 30 seconds
- **Data Points**: 5,000+ price records for analysis
- **Signal Generation**: Based on confidence thresholds

### Success Indicators
- ✅ All services show "Running" status
- ✅ Dashboard displays real-time data
- ✅ Trading signals appear with confidence levels
- ✅ Database stores all historical data
- ✅ No error messages in logs

## 🎯 Next Steps

1. **Monitor System**: Run for 24 hours to collect sufficient data
2. **Review Signals**: Analyze trading signals and confidence levels
3. **Paper Trading**: Test with paper trading mode enabled
4. **Strategy Tuning**: Adjust parameters based on performance
5. **Real Trading**: Enable real trading only after thorough testing

## 📞 Support

For issues or questions:
1. Check service logs for error messages
2. Verify all services are running in correct order
3. Ensure .env file is in project root directory
4. Confirm ports 8080 and 3000 are available
5. Test with paper trading before real trading

## 📄 License

This project is licensed under the MIT License - see the LICENSE file for details.

---

**⚠️ Disclaimer**: This software is for educational and research purposes. Trading cryptocurrencies involves substantial risk. Use at your own risk and never invest more than you can afford to lose. 