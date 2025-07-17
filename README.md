# 🚀 TIRADE - Advanced Solana Trading Bot

**TIRADE** is a sophisticated, AI-powered trading bot for Solana that combines traditional technical analysis with cutting-edge machine learning and neural networks to execute profitable trades on SOL/USDC pairs.

## 🧠 Core Features

### 🎯 **Intelligent Trading Engine**
- **Multi-wallet support** with round-robin position management
- **Smart exit strategy**: 0.8% technical exits + 1.2% profit targets
- **Single position per wallet** with 90% capital utilization
- **Real-time technical analysis** (RSI, SMA, momentum, volatility)
- **Jupiter DEX integration** for optimal swap execution

### 🤖 **Machine Learning Integration**
- **Adaptive signal enhancement** based on historical performance
- **Market regime detection** (trending, ranging, volatile)
- **Trade outcome prediction** using feature extraction
- **Continuous learning** from trade results
- **Performance-based confidence adjustment**

### 🧬 **Neural Network Enhancement**
- **Online learning algorithms** for pattern recognition
- **Real-time market adaptation** without retraining
- **Pattern matching** for entry/exit optimization
- **Risk assessment** using neural confidence scoring
- **Memory-based learning** from recent market behavior

### 📊 **Advanced Analytics**
- **Real-time dashboard** with live trading metrics
- **Position tracking** across multiple wallets
- **P&L monitoring** with detailed trade history
- **Technical indicator visualization**
- **ML/Neural performance insights**

## 🏗️ Architecture

```
┌─────────────────┐    ┌──────────────────┐    ┌─────────────────┐
│   Trading       │    │   Machine        │    │   Neural        │
│   Engine        │◄──►│   Learning       │◄──►│   Networks      │
│                 │    │   Strategy       │    │                 │
└─────────────────┘    └──────────────────┘    └─────────────────┘
         │                       │                       │
         ▼                       ▼                       ▼
┌─────────────────┐    ┌──────────────────┐    ┌─────────────────┐
│   Jupiter DEX   │    │   Database       │    │   Dashboard     │
│   Integration   │    │   Service        │    │   & Analytics   │
└─────────────────┘    └──────────────────┘    └─────────────────┘
```

## 🚀 Quick Start

### 1. Clone the Repository
```bash
git clone -b neural https://github.com/buzzkillb/tirade.git
cd tirade
```

### 2. Environment Setup
```bash
# Copy the example environment file
cp env.example .env

# Edit the .env file with your configuration
nano .env
```

**Required Environment Variables:**
```bash
# Trading Configuration
ENABLE_TRADING_EXECUTION=true          # Set to false for paper trading
POSITION_SIZE_PERCENTAGE=0.9           # Use 90% of wallet balance
SLIPPAGE_TOLERANCE=0.005               # 0.5% slippage tolerance
MIN_CONFIDENCE_THRESHOLD=0.7           # 70% minimum signal confidence

# Solana Configuration
SOLANA_RPC_URL=https://api.mainnet-beta.solana.com
SOLANA_PRIVATE_KEY=your_private_key_here

# Database Configuration
DATABASE_URL=http://localhost:3001

# Trading Pair
TRADING_PAIR=SOL/USDC

# Multi-wallet Support (optional)
WALLET_NAMES=Main,Secondary,Tertiary
WALLET_KEYS=key1,key2,key3
```

### 3. Initialize Database
```bash
# Make the script executable and run it
chmod +x init-database.sh
./init-database.sh
```

### 4. Start All Services
```bash
# Start all services in screen sessions
chmod +x start_all_screen.sh
./start_all_screen.sh
```

This will start:
- **Database Service** (Port 3001)
- **Price Feed Service** (Port 3002) 
- **Trading Logic Engine**
- **Web Dashboard** (Port 8080)

### 5. Access Dashboard
Open your browser and navigate to:
```
http://localhost:8080
```

## 🎯 Trading Strategy

### **Smart Exit Conditions**
The bot uses a sophisticated multi-tier exit strategy:

1. **Technical Protection (0.8% threshold)**
   - Exit early when RSI > 70 (overbought) + 0.8% profit
   - Exit when momentum decay detected + 0.8% profit

2. **Profit Target (1.2%)**
   - Take solid gains when no technical issues present

3. **Risk Management (1% stop loss)**
   - Tight risk control to preserve capital

### **Position Management**
- **Single position per wallet** to prevent over-leveraging
- **90% capital utilization** for maximum efficiency
- **Database-driven position tracking** (not balance-based)
- **Round-robin wallet rotation** for multi-wallet setups

## 🤖 Machine Learning Features

### **Signal Enhancement**
- Analyzes historical trade performance
- Adjusts signal confidence based on market conditions
- Learns from successful/failed trades
- Adapts to changing market regimes

### **Feature Extraction**
- Price momentum and volatility patterns
- Technical indicator combinations
- Market microstructure analysis
- Time-based pattern recognition

### **Continuous Learning**
- Updates models after each trade
- No offline retraining required
- Real-time adaptation to market changes
- Performance-based strategy adjustment

## 🧬 Neural Network Integration

### **Online Learning**
- Processes market data in real-time
- Adapts without historical data requirements
- Memory-efficient pattern storage
- Incremental learning from new observations

### **Pattern Recognition**
- Identifies recurring market patterns
- Matches current conditions to historical outcomes
- Confidence-weighted decision making
- Risk assessment through neural scoring

### **Adaptive Algorithms**
- Self-adjusting learning rates
- Pattern memory management
- Confidence threshold optimization
- Real-time performance monitoring

## 📊 Monitoring & Management

### **Screen Sessions**
View running services:
```bash
screen -ls
```

Attach to specific services:
```bash
screen -r database-service
screen -r price-feed
screen -r trading-logic
screen -r dashboard
```

### **Logs**
Monitor real-time logs:
```bash
tail -f logs/trading-logic.log
tail -f logs/database-service.log
```

### **Stop Services**
```bash
./stop_all_screen.sh
```

## 🔧 Configuration

### **Trading Parameters**
- `POSITION_SIZE_PERCENTAGE`: Percentage of wallet to use (0.9 = 90%)
- `MIN_CONFIDENCE_THRESHOLD`: Minimum signal confidence (0.7 = 70%)
- `SLIPPAGE_TOLERANCE`: Maximum acceptable slippage (0.005 = 0.5%)

### **ML/Neural Settings**
- Automatic adaptation based on performance
- No manual tuning required
- Self-optimizing parameters
- Real-time learning rate adjustment

### **Multi-Wallet Setup**
Configure multiple wallets for diversification:
```bash
WALLET_NAMES=Wallet1,Wallet2,Wallet3
WALLET_KEYS=key1,key2,key3
```

## 🛡️ Security & Risk Management

### **Risk Controls**
- **1% stop loss** for tight risk management
- **Single position limit** prevents over-exposure
- **Confidence thresholds** filter low-quality signals
- **Slippage protection** on all trades

### **Security Features**
- Private keys stored in environment variables
- No sensitive data in code or logs
- Secure RPC connections
- Database isolation

## 📈 Performance Metrics

The dashboard provides real-time insights:
- **Live P&L tracking**
- **Win rate statistics**
- **Average trade duration**
- **ML/Neural performance scores**
- **Technical indicator status**
- **Position management overview**

## 🔄 Maintenance

### **Regular Tasks**
- Monitor dashboard for performance
- Check logs for any errors
- Verify wallet balances
- Review trade history

### **Updates**
```bash
git pull origin neural
./stop_all_screen.sh
./start_all_screen.sh
```

## 🆘 Troubleshooting

### **Common Issues**
1. **Database connection errors**: Check if database service is running
2. **RPC timeouts**: Verify SOLANA_RPC_URL is accessible
3. **Trading execution failures**: Check wallet balance and private key
4. **Dashboard not loading**: Ensure port 8080 is available

### **Debug Mode**
Enable detailed logging:
```bash
RUST_LOG=debug ./start_all_screen.sh
```

## 📚 Technical Details

### **Built With**
- **Rust** - High-performance systems programming
- **Tokio** - Async runtime for concurrent operations
- **Jupiter** - Solana DEX aggregator for optimal swaps
- **PostgreSQL** - Reliable data persistence
- **WebSocket** - Real-time dashboard updates

### **Key Components**
- **Trading Engine**: Core logic and strategy execution
- **ML Strategy**: Machine learning signal enhancement
- **Neural Enhancement**: Online learning and pattern recognition
- **Database Service**: Data persistence and analytics
- **Price Feed**: Real-time market data collection
- **Dashboard**: Web-based monitoring interface

---

## ⚠️ Disclaimer

This software is for educational and research purposes. Trading cryptocurrencies involves substantial risk of loss. Never trade with funds you cannot afford to lose. The developers are not responsible for any financial losses incurred through the use of this software.

## 📄 License

This project is licensed under the MIT License - see the LICENSE file for details.

---

**Happy Trading! 🚀**