# Tirade Trading Bot - Troubleshooting Guide

## ✅ Fixed: "No such file or directory (os error 2)" Error

### Problem
The trading logic was failing with:
```
ERROR trading_logic::trading_engine: ❌ BUY signal execution error: No such file or directory (os error 2)
```

### Root Cause
The trading logic depends on a `transaction` binary from the `solana-trading-bot` component that wasn't being built before starting the services.

### Solution
Updated `start_all_screen.sh` to build all necessary binaries before starting services:

1. **Transaction Binary**: `solana-trading-bot/target/debug/transaction`
2. **Database Service**: `database-service/target/debug/database-service`
3. **Price Feed**: `price-feed/target/debug/price-feed`
4. **Trading Logic**: `trading-logic/target/debug/trading-logic`
5. **Dashboard**: `dashboard/target/debug/dashboard`

### Prevention
Always run `./start_all_screen.sh` which now includes automatic building of all dependencies.

## 🔧 Useful Commands

### Check Service Status
```bash
# List all screen sessions
screen -list

# Attach to a specific service
screen -r tirade-trading    # Trading logic
screen -r tirade-db         # Database service
screen -r tirade-price      # Price feed
screen -r tirade-dashboard  # Dashboard

# Detach from session: Ctrl+A, then D
```

### Monitor Services
```bash
# Check trading logic output
screen -S tirade-trading -X hardcopy /tmp/trading.txt && cat /tmp/trading.txt

# Check database service
curl http://localhost:8080/health

# Check dashboard
curl http://localhost:3000/

# Check price feed logs
screen -S tirade-price -X hardcopy /tmp/price.txt && cat /tmp/price.txt
```

### Manual Building (if needed)
```bash
# Build all components manually
cd solana-trading-bot && cargo build --bin transaction && cd ..
cd database-service && cargo build && cd ..
cd price-feed && cargo build && cd ..
cd trading-logic && cargo build && cd ..
cd dashboard && cargo build && cd ..
```

### Stop All Services
```bash
# Kill all tirade screen sessions
screen -S tirade-db -X quit
screen -S tirade-price -X quit
screen -S tirade-trading -X quit
screen -S tirade-dashboard -X quit

# Or use the stop script (if available)
./stop_all_screen.sh
```

## 🚨 Common Issues

### 1. Port Already in Use
```bash
# Check what's using a port
lsof -i :8080  # Database service
lsof -i :8081  # Price feed
lsof -i :3000  # Dashboard

# Kill process using port
kill -9 <PID>
```

### 2. Environment Variables Not Loaded
```bash
# Ensure .env file exists in project root
ls -la .env

# Check environment variables
source .env && env | grep SOLANA
```

### 3. Database Issues
```bash
# Check database file exists
ls -la data/trading_bot.db

# Test database connection
curl http://localhost:8080/prices/SOL-USDC/history?hours=1
```

### 4. Build Failures
```bash
# Clean and rebuild all
cargo clean
./start_all_screen.sh

# Check Rust installation
rustc --version
cargo --version
```

## 📊 Service URLs
- **Dashboard**: http://localhost:3000
- **Database API**: http://localhost:8080
- **Price Feed**: Internal (port 8081)
- **Trading Logic**: Background service (no web interface)

## ⚡ Quick Recovery
If anything goes wrong:
```bash
# Stop everything
pkill -f tirade

# Restart everything
./start_all_screen.sh
``` 