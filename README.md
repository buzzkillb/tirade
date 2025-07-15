# 🤖 Tirade - AI-Powered Solana Trading Bot

A sophisticated Rust-based trading bot for Solana with advanced machine learning capabilities, real-time market analysis, and automated position management.

[![Rust](https://img.shields.io/badge/Rust-80.7%25-orange)](https://github.com/buzzkillb/tirade)
[![License](https://img.shields.io/badge/License-MIT-green)](https://github.com/buzzkillb/tirade/blob/master/LICENSE)
[![GitHub](https://img.shields.io/badge/GitHub-buzzkillb%2Ftirade-blue)](https://github.com/buzzkillb/tirade)

## 🎯 Overview

Tirade is a high-frequency trading bot designed for the Solana ecosystem, featuring:

- **Advanced ML Strategy**: Real-time performance-based confidence adjustments
- **Multi-Source Price Feeds**: Pyth Network, Jupiter, and Coinbase integration
- **Technical Analysis**: RSI, SMA, volatility, and market regime detection
- **Risk Management**: Dynamic position sizing and stop-loss/take-profit
- **Real-Time Dashboard**: Live monitoring with technical indicators
- **Paper Trading Mode**: Safe testing environment

## 🧠 Trading Logic & Machine Learning

### **Core Strategy Architecture**

The bot uses a **dual-strategy approach** combining traditional technical analysis with machine learning enhancements:

#### **1. Technical Analysis Strategy (Option 2)**
```rust
// Simplified strategy using only RSI and Moving Average Trend
// 1. RSI Overbought/Oversold
if rsi_fast > dynamic_thresholds.rsi_overbought {
    signal_type = SignalType::Sell;
    confidence += 0.5;
} else if rsi_fast < dynamic_thresholds.rsi_oversold {
    signal_type = SignalType::Buy;
    confidence += 0.5;
}

// 2. Moving Average Trend
if current_price > sma_short && rsi_fast >= 40.0 && rsi_fast <= 60.0 {
    signal_type = SignalType::Buy;
    confidence += 0.3;
}
```

**Key Features:**
- **RSI14 Analysis**: Consistent RSI14 calculations across strategy and dashboard
- **Dynamic Thresholds**: Adaptive to market conditions (trending, ranging, volatile, consolidating)
- **Moving Average Confirmation**: Price vs SMA20 with RSI neutral zone validation
- **Confidence Scoring**: 0.0-1.0 scale with 35% minimum threshold

#### **2. Machine Learning Enhancement**

The ML system provides **real-time performance-based adjustments**:

```rust
// ML Features (4 essential metrics)
pub struct MLFeatures {
    pub rsi_fast: f64,           // Current RSI (normalized 0-1)
    pub win_rate: f64,           // Recent win rate (0-1)
    pub consecutive_losses: f64,  // Number of consecutive losses
    pub volatility: f64,         // Current volatility
}
```

**ML Learning Process:**
1. **Trade Recording**: Every closed position is recorded with market context
2. **Performance Analysis**: Win rate calculated from last 10 trades
3. **Risk Assessment**: Consecutive losses and volatility analysis
4. **Confidence Adjustment**: Real-time ML enhancements to base strategy

**ML Confidence Adjustments:**
```rust
// Conservative ML adjustments
if prediction.win_rate > 0.7 {
    enhanced_signal.confidence += 0.05; // Small boost for consistent wins
} else if prediction.win_rate < 0.3 {
    enhanced_signal.confidence -= 0.05; // Small reduction for losses
}

if prediction.consecutive_losses > 3.0 {
    enhanced_signal.confidence -= 0.1; // Bigger reduction after many losses
}
```

### **Market Regime Detection**

The system automatically detects market conditions:

```rust
pub enum MarketRegime {
    Consolidating,  // Low volatility, sideways movement
    Trending,       // Strong directional movement
    Volatile,       // High volatility, unpredictable
    Unknown,        // Insufficient data
}
```

**Regime-Based Adjustments:**
- **Trending**: Higher confidence for trend-following signals
- **Volatile**: Reduced position sizes and confidence
- **Consolidating**: Conservative approach with lower thresholds

### **Risk Management System**

#### **Position Management**
- **Single Position**: Only one active position at a time
- **Dynamic Sizing**: Based on ML confidence and market conditions
- **Stop Loss**: -2% automatic exit
- **Take Profit**: +1.5% automatic exit

#### **ML-Based Position Sizing**
```rust
fn calculate_optimal_position_size(&self, features: &MLFeatures) -> f64 {
    let mut size = self.max_position_size; // Start with max size
    
    // Reduce size based on risk factors
    if features.consecutive_losses > 2.0 { size *= 0.5; } // 50% reduction
    if features.consecutive_losses > 4.0 { size *= 0.3; } // 70% reduction
    if features.volatility > 0.08 { size *= 0.6; } // 40% reduction in high volatility
    if features.win_rate < 0.4 { size *= 0.7; } // 30% reduction with poor performance
    
    size.max(0.05).min(self.max_position_size) // Min 5%, Max 90%
}
```

## 🏗️ System Architecture

### **Microservices Design**

```
┌─────────────────┐    ┌─────────────────┐    ┌─────────────────┐
│   Price Feed    │    │ Trading Logic   │    │   Dashboard     │
│                 │    │                 │    │                 │
│ • Pyth Network  │    │ • Strategy      │    │ • Real-time UI  │
│ • Jupiter       │    │ • ML Engine     │    │ • Charts        │
│ • Coinbase      │    │ • Risk Mgmt     │    │ • Signals       │
└─────────────────┘    └─────────────────┘    └─────────────────┘
         │                       │                       │
         └───────────────────────┼───────────────────────┘
                                 │
                    ┌─────────────────┐
                    │   Database      │
                    │                 │
                    │ • SQLite        │
                    │ • Trade History │
                    │ • ML Data       │
                    └─────────────────┘
```

### **Data Flow**

1. **Price Collection**: Multi-source price feeds every 1-10 seconds
2. **Technical Analysis**: RSI, SMA, volatility calculations
3. **Strategy Execution**: Signal generation with confidence scoring
4. **ML Enhancement**: Performance-based adjustments
5. **Position Management**: Risk-controlled trade execution
6. **Data Storage**: Historical trade and ML data persistence

## 🚀 Quick Start

### **Prerequisites**

```bash
# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source ~/.cargo/env

# Install system dependencies
sudo apt update
sudo apt install -y screen sqlite3 curl

# Clone the repository
git clone https://github.com/buzzkillb/tirade.git
cd tirade
```

### **1. Database Initialization**

```bash
# Initialize the database schema and tables
./init_database.sh
```

This script:
- Creates SQLite database with all required tables
- Sets up ML trade history storage
- Initializes technical indicators tables
- Configures performance metrics tracking

### **2. Environment Configuration**

```bash
# Copy example environment file
cp env.example .env

# Edit with your configuration
nano .env
```

**Required Environment Variables:**
```bash
# Solana Configuration
SOLANA_PRIVATE_KEY="your_private_key_here"
SOLANA_RPC_URL="https://api.mainnet-beta.solana.com"

# Trading Configuration
TRADING_PAIR="SOL/USDC"
MIN_CONFIDENCE_THRESHOLD=0.35
POSITION_SIZE_PERCENTAGE=0.5

# Database Configuration
DATABASE_URL="sqlite:data/trading_bot.db"

# Paper Trading (Recommended for testing)
ENABLE_TRADING_EXECUTION=false
```

### **3. Start All Services**

```bash
# Start all services in screen sessions
./start_all_screen.sh
```

This script starts:
- **Database Service** (Port 8080)
- **Price Feed** (Port 8081)
- **Trading Logic** (Port 8082)
- **Dashboard** (Port 3000)

### **4. Monitor the System**

```bash
# Check service status
./check_trading_status.sh

# View trading logs
tail -f logs/trading_logic.log

# Access dashboard
# Open http://localhost:3000 in your browser
```

## 📊 Dashboard Features

### **Real-Time Monitoring**

- **Live Price Charts**: SOL/USDC with technical indicators
- **Trading Signals**: Buy/Sell/Hold with confidence levels
- **Position Management**: Active positions with P&L tracking
- **ML Metrics**: Win rate, consecutive losses, market regime
- **System Status**: Service health and performance

### **Technical Indicators Display**

- **RSI14**: Relative Strength Index (14-period)
- **SMA20/SMA50**: Simple Moving Averages
- **Volatility**: 24-hour price volatility
- **Market Regime**: Trending/Consolidating/Volatile

### **ML Performance Tracking**

- **Win Rate**: Percentage of profitable trades
- **Consecutive Losses**: Current losing streak
- **Risk Score**: ML-calculated risk assessment
- **Confidence Adjustments**: Real-time ML enhancements

## 🔧 Advanced Configuration

### **Trading Parameters**

```bash
# Strategy Configuration
MIN_DATA_POINTS=60              # Minimum data points before analysis
CHECK_INTERVAL_SECS=30          # Analysis frequency (30 seconds)
CONFIDENCE_THRESHOLD=0.35       # Minimum confidence for trades
SIGNAL_COOLDOWN_SECS=300        # Cooldown between signals (5 minutes)

# Risk Management
POSITION_SIZE_PERCENTAGE=0.5    # % of wallet per trade
MAX_POSITION_SIZE=100.0         # Maximum position size
TAKE_PROFIT_PERCENT=1.5         # Dynamic take profit
STOP_LOSS_PERCENT=2.0           # Dynamic stop loss

# ML Configuration
ML_ENABLED=true                 # Enable ML enhancements
ML_MIN_CONFIDENCE=0.35          # ML confidence threshold
ML_MAX_POSITION_SIZE=0.9        # ML position size limit
```

### **Technical Indicators**

```bash
# RSI Configuration (Updated for consistency)
RSI_FAST_PERIOD=14             # RSI14 for consistency
RSI_SLOW_PERIOD=21             # RSI21 for divergence
RSI_OVERSOLD=30                # Oversold threshold
RSI_OVERBOUGHT=70              # Overbought threshold

# Moving Averages
SMA_SHORT_PERIOD=20            # Short-term SMA
SMA_LONG_PERIOD=50             # Long-term SMA

# Volatility
VOLATILITY_WINDOW=20           # Volatility calculation window
```

## 🛠️ Troubleshooting

### **Common Issues**

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
./init_database.sh

# Check database file
ls -la data/trading_bot.db
```

#### **Trading Logic Issues**
```bash
# Check logs with enhanced debugging
tail -f logs/trading_logic.log

# Verify wallet
solana balance

# Test ML trade queries
./query_trades_ml.sh --pair SOLUSDC --stats
```

### **Performance Optimization**

```bash
# Increase system limits
echo 'fs.file-max = 65536' | sudo tee -a /etc/sysctl.conf
sudo sysctl -p

# Optimize SQLite
echo 'PRAGMA journal_mode=WAL;' | sqlite3 data/trading_bot.db
```

## 📈 Performance Metrics

### **Expected Behavior**

- **Price Updates**: Every 1-10 seconds (multi-source)
- **Trading Analysis**: Every 30 seconds
- **Dashboard Refresh**: Every 2 seconds
- **Signal Generation**: Based on confidence thresholds
- **Position Duration**: 5 minutes to several hours

### **Success Indicators**

- ✅ All services show "Running" status
- ✅ Dashboard displays real-time data
- ✅ Trading signals appear with confidence levels
- ✅ ML metrics show stable win rates
- ✅ No sudden confidence jumps
- ✅ Consistent RSI values across dashboard and strategy

## 🔒 Security Best Practices

### **Environment Security**
1. **Never commit private keys**: `.env` files are gitignored
2. **Use paper trading first**: `ENABLE_TRADING_EXECUTION=false`
3. **Start with small amounts**: `POSITION_SIZE_PERCENTAGE=0.1`
4. **Monitor continuously**: Use dashboard and logs

### **System Security**
1. **SSH Access**: Use SSH keys, not passwords
2. **Firewall**: Only open necessary ports
3. **Regular Updates**: Keep system and dependencies updated
4. **Backup**: Regular database backups

## 🤝 Contributing

1. Fork the repository
2. Create a feature branch: `git checkout -b feature-name`
3. Make your changes
4. Test thoroughly with paper trading
5. Submit a pull request

### **Development Setup**

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

1. Check the [Troubleshooting](TROUBLESHOOTING.md) section
2. Review service logs for error messages
3. Open an issue on [GitHub](https://github.com/buzzkillb/tirade)
4. Join our community discussions

---

**Built with ❤️ in Rust for the Solana ecosystem**

[GitHub Repository](https://github.com/buzzkillb/tirade) | [Issues](https://github.com/buzzkillb/tirade/issues) | [Discussions](https://github.com/buzzkillb/tirade/discussions) 