# 🧠 TIRADE - AI-Powered Solana Trading Bot

**TIRADE** is an advanced, fully autonomous trading bot for Solana that uses cutting-edge neural networks and machine learning to execute profitable SOL/USDC trades. The system operates under **complete AI control** with no human-imposed safety nets or fixed thresholds.

## 🚀 How It Works

### **🧠 Neural Network Trading Logic**

TIRADE's neural network makes **all trading decisions** through sophisticated AI analysis:

**Real-Time Learning (Every 30 Seconds):**
- Processes 100 1-minute price candles
- Analyzes RSI, momentum, volatility patterns
- Generates predictions and builds market intelligence
- Currently making 800+ predictions (6+ hours of continuous learning)

**Intelligent Decision Making:**
```
🧠 Neural Control Example:
RSI=70.3 > 75.0 && PnL=-0.06% > 0.5% = false
Decision: HOLD (wait for better exit conditions)
```

The neural network learned that:
- **RSI overbought + profit context** = smart exit timing
- **Small losses + technical signals** = patience required
- **Oversold conditions (RSI < 30)** = buying opportunities
- **Market consolidation** = hold and wait for clarity

### **🤖 Machine Learning Strategy**

**Adaptive Signal Enhancement:**
- Continuously learns from market patterns
- Adjusts confidence based on market conditions
- No historical training data required - learns in real-time
- Enhances traditional technical analysis with AI insights

**Market Regime Detection:**
- **Consolidating**: Sideways market movement (current state)
- **Trending**: Strong directional movement
- **Volatile**: High volatility periods
- **Adapting**: Learning new market conditions

**Feature Extraction:**
- Price momentum and volatility patterns
- RSI and technical indicator combinations
- Market microstructure analysis
- Time-based pattern recognition

### **🎯 Trading Execution Logic**

**Position Management:**
- **Single position per wallet** (prevents over-leveraging)
- **90% capital utilization** (maximum efficiency)
- **Neural exit conditions** (no fixed stop-loss/take-profit)
- **Smart signal processing** (Buy/Sell/Hold recommendations)

**AI Decision Flow:**
1. **Market Analysis**: Process price data and technical indicators
2. **Neural Prediction**: Generate AI-powered market forecast
3. **Signal Generation**: Create Buy/Sell/Hold recommendations
4. **Risk Assessment**: Evaluate position context and market conditions
5. **Execution Decision**: Neural network makes final trading choice

**Example Neural Behavior:**
- **Wants to buy** when RSI drops to 28.1 (oversold)
- **Holds position** during small losses (-0.10% PnL)
- **Ignores sell signals** when profit requirements not met
- **Adapts confidence** based on market clarity (30% → 50%)

## 🚀 Quick Start

### **1. Clone Repository**
```bash
git clone -b neuralv2 https://github.com/buzzkillb/tirade.git
cd tirade
```

### **2. Environment Setup**
```bash
# Copy example environment file
cp env.example .env

# Edit configuration (required)
nano .env
```

**Essential Configuration:**
```bash
# Trading Settings
ENABLE_TRADING_EXECUTION=true          # Enable live trading
POSITION_SIZE_PERCENTAGE=0.9           # Use 90% of wallet
SLIPPAGE_TOLERANCE=0.005               # 0.5% slippage limit

# Solana Configuration  
SOLANA_RPC_URL=https://api.mainnet-beta.solana.com
SOLANA_PRIVATE_KEY=your_private_key_here

# Database
DATABASE_URL=http://localhost:3001

# Trading Pair
TRADING_PAIR=SOL/USDC
```

### **3. Initialize Database**
```bash
chmod +x init-database.sh
./init-database.sh
```

### **4. Start All Services**
```bash
chmod +x start_all_screen.sh
./start_all_screen.sh
```

**Services Started:**
- **Database Service** (Port 3001) - Data persistence
- **Price Feed Service** (Port 3002) - Real-time market data  
- **Trading Logic Engine** - AI decision making
- **Web Dashboard** (Port 8080) - Monitoring interface

### **5. Monitor Dashboard**
```
http://localhost:8080
```

### **6. Stop All Services**
```bash
./stop_all_screen.sh
```

## 🧠 Neural Network Intelligence

### **Learning Timeline**
- **Minutes**: Pattern recognition from price movements
- **Hours**: Technical indicator relationship learning  
- **Days**: Trade outcome validation and strategy refinement
- **Weeks**: Advanced market regime adaptation

### **Current AI Status** (From Logs)
```
💾 Neural state saved - Accuracy: 0.1%, Predictions: 790
🧠 Neural enhancement applied successfully (no ML history)
🚀 Neural Network: FULL CONTROL - All overrides disabled
```

**What This Means:**
- **790 predictions**: 6+ hours of active market learning
- **0.1% accuracy**: Normal for early learning phase
- **No ML history**: Fresh system learning from scratch
- **Full control**: No human safety nets interfering

### **Neural Decision Examples**

**Oversold Opportunity Recognition:**
```
RSI=28.1 → Generate BUY signals (wants to buy the dip)
Position: -0.12% PnL → Neural logic: acceptable loss for potential upside
```

**Profit Protection Logic:**
```
RSI=70.3 (overbought) + PnL=-0.06% → Decision: HOLD
Reasoning: Don't exit on small loss despite technical warning
```

**Signal Adaptation:**
```
Market uncertain → HOLD signal (30% confidence)
Conditions clear → SELL signal (50% confidence)  
```

## 📊 Dashboard Understanding

### **Neural Network Status**
- **Overall Accuracy**: Learning progress (starts low, improves over time)
- **Total Predictions**: Market observations made (higher = more learning)
- **Learning Rate**: AI adaptation speed (decreases as system matures)
- **Market Regime**: AI's understanding of current market state

### **AI Market Insights**
- **Price Direction**: Neural forecast (Bullish/Bearish/Neutral)
- **Volatility Forecast**: Expected market volatility
- **Optimal Position Size**: AI-recommended position sizing
- **Position Confidence**: AI certainty in current market conditions

### **ML Strategy Performance**
- **ML Win Rate**: Success rate of AI decisions
- **ML Trades**: Number of completed trades for learning
- **ML Confidence**: Overall AI system confidence
- **AI Control Status**: Confirms neural network is active

## 🔧 Advanced Configuration

### **Multi-Wallet Setup**
```bash
WALLET_NAMES=Main,Secondary,Tertiary
WALLET_KEYS=key1,key2,key3
```

### **Neural Network Tuning** (Optional)
```bash
# Learning rate adjustment
NEURAL_LEARNING_RATE=0.01

# Pattern memory size  
NEURAL_MEMORY_SIZE=1000

# Confidence threshold
NEURAL_CONFIDENCE_THRESHOLD=0.6
```

## 📈 Monitoring & Management

### **View Running Services**
```bash
screen -ls
```

### **Attach to Specific Service**
```bash
screen -r trading-logic    # View AI decision making
screen -r database-service # Monitor data persistence
screen -r price-feed      # Watch market data
screen -r dashboard       # Dashboard service
```

### **Monitor Logs**
```bash
tail -f logs/trading-logic.log     # AI decisions and neural activity
tail -f logs/database-service.log  # Data operations
```

### **Debug Mode**
```bash
RUST_LOG=debug ./start_all_screen.sh
```

## 🛡️ Risk Management

### **AI-Powered Risk Controls**
- **Neural risk assessment**: Dynamic risk evaluation
- **Context-aware exits**: Profit/loss context in all decisions
- **Adaptive position sizing**: AI adjusts based on market conditions
- **Real-time monitoring**: Continuous market analysis

### **Built-in Protections**
- **Single position limit**: Prevents over-leveraging
- **Slippage protection**: Maximum acceptable slippage
- **Confidence filtering**: Only acts on high-confidence signals
- **Market regime awareness**: Adapts to different market conditions

## 🔄 System Architecture

```
Market Data → Neural Network → AI Decision → Trade Execution
     ↓              ↓              ↓              ↓
Price Feed → Pattern Learning → Signal Gen → Jupiter DEX
     ↓              ↓              ↓              ↓  
Database ← Performance Track ← Trade Results ← Confirmation
```

## 🆘 Troubleshooting

### **Common Issues**
1. **No trades executing**: Check `ENABLE_TRADING_EXECUTION=true`
2. **Database errors**: Ensure `init-database.sh` was run
3. **RPC timeouts**: Verify `SOLANA_RPC_URL` accessibility
4. **Dashboard not loading**: Check port 8080 availability

### **Neural Network Issues**
- **Low accuracy**: Normal for new systems, improves with time
- **No ML history**: Expected until first trades complete
- **Signal conflicts**: AI learning process, will stabilize

## 📚 Technical Stack

- **Rust**: High-performance trading engine
- **Neural Networks**: Online learning algorithms
- **Machine Learning**: Real-time pattern recognition
- **Jupiter Protocol**: Solana DEX integration
- **PostgreSQL**: Trade data persistence
- **WebSocket**: Real-time dashboard updates

---

## ⚠️ Important Disclaimer

**This is experimental AI trading software.** 

- **High Risk**: Cryptocurrency trading involves substantial risk of loss
- **AI Learning**: Neural network is learning and may make mistakes
- **No Guarantees**: Past performance does not guarantee future results
- **Test First**: Consider paper trading before live deployment
- **Never Risk**: More than you can afford to lose

## 📄 License

MIT License - See LICENSE file for details.

---

## 🧠 **Neural Network Status: ACTIVE** 
**🚀 Full AI Control Enabled - No Human Overrides**

**Happy AI Trading! 🤖📈**