# Trading System Testing Guide

This guide covers comprehensive testing strategies for the Solana trading bot, including full trading scenarios, balance verification, and profit/loss tracking.

## 🚀 Quick Start Testing

### 1. Basic Test Script
```bash
# Run the comprehensive test script (dry-run mode)
./test_trading_scenario.sh

# Run with custom amount
./test_trading_scenario.sh --amount 0.50

# Run in live mode (WARNING: Real trades will be executed!)
./test_trading_scenario.sh --live
```

### 2. Rust Unit Tests
```bash
# Run all trading tests
cd trading-logic
cargo test

# Run specific test
cargo test test_full_trading_scenario

# Run tests with output
cargo test -- --nocapture
```

## 📋 Test Scenarios

### Scenario 1: Full Trading Cycle (Recommended)
**Purpose**: Test complete USDC → SOL → USDC cycle with PnL tracking

**Steps**:
1. ✅ Get initial balances
2. ✅ Execute USDC → SOL swap ($1.00)
3. ✅ Execute SOL → USDC swap (all received SOL)
4. ✅ Calculate PnL and verify accuracy
5. ✅ Verify balance consistency
6. ✅ Test trading signal integration

**Expected Outcomes**:
- Both swaps execute successfully
- PnL calculation is accurate (may be negative due to fees)
- Balance changes are consistent
- All services remain healthy

### Scenario 2: Balance Verification
**Purpose**: Ensure balance checking works correctly

**Steps**:
1. ✅ Get wallet balances before trades
2. ✅ Execute small test trade
3. ✅ Get wallet balances after trades
4. ✅ Verify balance changes match expectations

**Expected Outcomes**:
- Balance retrieval works consistently
- Changes are within expected tolerances
- No balance inconsistencies

### Scenario 3: Trading Logic Integration
**Purpose**: Test signal generation and execution

**Steps**:
1. ✅ Verify trading logic is running
2. ✅ Check signal generation
3. ✅ Test signal execution (dry-run)
4. ✅ Verify signal storage in database

**Expected Outcomes**:
- Trading logic process is active
- Signals are being generated
- Signal execution works correctly
- Database integration functions properly

## 🔧 Test Configuration

### Environment Variables
```bash
# Required for testing
SOLANA_PRIVATE_KEY=your_private_key_here
SOLANA_RPC_URL=https://api.mainnet-beta.solana.com

# Trading execution settings
ENABLE_TRADING_EXECUTION=false  # Set to true for live testing
POSITION_SIZE_PERCENTAGE=0.9
SLIPPAGE_TOLERANCE=0.005
MIN_CONFIDENCE_THRESHOLD=0.7
```

### Test Parameters
```bash
# Default test configuration
TEST_AMOUNT_USDC=1.00          # Small test amount
SLIPPAGE_TOLERANCE=0.005        # 0.5% slippage
MIN_CONFIDENCE=0.7              # 70% confidence threshold
DRY_RUN=true                    # Start with dry-run for safety
```

## 📊 PnL Testing Strategy

### Understanding Expected Losses
When testing with small amounts ($1), you should expect:
- **Small losses** due to transaction fees (typically $0.01-$0.05)
- **Slippage impact** (0.5% = $0.005 on $1 trade)
- **Total expected loss**: $0.015-$0.055 on $1 trade

### PnL Calculation Verification
```rust
// Example PnL calculation
let usdc_spent = 1.00;
let usdc_received = 0.945;  // After fees and slippage
let pnl = usdc_received - usdc_spent;  // -0.055 USDC
let pnl_percent = (pnl / usdc_spent) * 100.0;  // -5.5%
```

### Acceptable Loss Ranges
- **$1 trade**: -$0.01 to -$0.06 loss (1-6%)
- **$10 trade**: -$0.10 to -$0.30 loss (1-3%)
- **$100 trade**: -$1.00 to -$2.00 loss (1-2%)

## 🔍 Balance Verification

### Pre-Trade Checks
```bash
# Get initial balances
cd solana-trading-bot
cargo run --bin transaction -- --amount-usdc 0.01 --direction usdc-to-sol --dry-run
```

### Post-Trade Verification
```bash
# Verify final balances
cargo run --bin transaction -- --amount-usdc 0.01 --direction usdc-to-sol --dry-run
```

### Balance Consistency Rules
1. **SOL balance** should increase after USDC→SOL swap
2. **USDC balance** should decrease after USDC→SOL swap
3. **SOL balance** should decrease after SOL→USDC swap
4. **USDC balance** should increase after SOL→USDC swap
5. **Total value** should be within fee tolerance

## 🛡️ Safety Measures

### Dry-Run Testing
```bash
# Always start with dry-run
./test_trading_scenario.sh

# Only use live mode after thorough testing
./test_trading_scenario.sh --live
```

### Small Amount Testing
```bash
# Start with $1 trades
./test_trading_scenario.sh --amount 1.00

# Gradually increase for confidence
./test_trading_scenario.sh --amount 5.00
./test_trading_scenario.sh --amount 10.00
```

### Service Health Checks
```bash
# Verify all services are running
curl http://localhost:8080/health  # Database service
curl http://localhost:3000          # Dashboard
pgrep -f "trading-logic"           # Trading logic
```

## 📈 Performance Testing

### Test Different Amounts
```bash
# Test various trade sizes
./test_trading_scenario.sh --amount 0.50   # $0.50
./test_trading_scenario.sh --amount 1.00   # $1.00
./test_trading_scenario.sh --amount 5.00   # $5.00
./test_trading_scenario.sh --amount 10.00  # $10.00
```

### Test Different Market Conditions
- **High volatility**: Test during market swings
- **Low liquidity**: Test during off-hours
- **Network congestion**: Test during peak times

## 🐛 Troubleshooting

### Common Issues

#### 1. "Transaction binary not found"
```bash
# Build the transaction binary
cd solana-trading-bot
cargo build --bin transaction
```

#### 2. "Insufficient balance"
```bash
# Check your wallet has enough funds
cd solana-trading-bot
cargo run --bin transaction -- --amount-usdc 0.01 --direction usdc-to-sol --dry-run
```

#### 3. "RPC connection failed"
```bash
# Try alternative RPC endpoints
export SOLANA_RPC_URL=https://solana-api.projectserum.com
# or
export SOLANA_RPC_URL=https://rpc.ankr.com/solana
```

#### 4. "Services not running"
```bash
# Start all services
./start_all_screen.sh
```

### Debug Mode
```bash
# Run with verbose logging
RUST_LOG=debug cargo test test_full_trading_scenario -- --nocapture
```

## 📝 Test Reports

### Generated Reports
The test script generates detailed reports:
```
trading_test_report_20241201_143022.txt
```

### Report Contents
- Test configuration
- Initial and final balances
- PnL analysis
- Service health status
- Error logs (if any)

### Interpreting Results
- **✅ Success**: All steps completed, PnL within expected range
- **⚠️ Warning**: Minor issues (e.g., high slippage, service delays)
- **❌ Failure**: Critical issues (e.g., transaction failures, balance inconsistencies)

## 🔄 Continuous Testing

### Automated Testing
```bash
# Run tests every hour
crontab -e
# Add: 0 * * * * cd /path/to/tirade && ./test_trading_scenario.sh >> test_logs.txt 2>&1
```

### Monitoring Dashboard
- Check dashboard at `http://localhost:3000`
- Monitor real-time price feeds
- Track signal generation
- Verify trade execution

## 🎯 Best Practices

1. **Always start with dry-run mode**
2. **Use small test amounts** ($1-$10)
3. **Verify all services are running** before testing
4. **Check balances before and after** each test
5. **Expect small losses** due to fees and slippage
6. **Monitor logs** for any errors or warnings
7. **Test regularly** to catch issues early
8. **Document any anomalies** for future reference

## 🚨 Emergency Procedures

### If Live Trading Goes Wrong
1. **Stop all services**: `pkill -f "trading"`
2. **Check balances**: Verify current wallet state
3. **Review logs**: Identify what went wrong
4. **Fix issues**: Address root cause
5. **Test thoroughly**: Run full test suite before resuming

### Contact Information
- **GitHub Issues**: Report bugs and feature requests
- **Documentation**: Check README files for updates
- **Community**: Join Discord/Telegram for support

---

**Remember**: Always test thoroughly before running live trades. Start small and gradually increase amounts as you gain confidence in the system. 