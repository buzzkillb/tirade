#!/bin/bash

# Tirade Trading Status Diagnostic Script
# This script helps diagnose why trades aren't being made

set -e

echo "🔍 Tirade Trading Status Diagnostic"
echo "==================================="
echo ""

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

print_status() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

print_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

print_warning() {
    echo -e "${YELLOW}[WARNING]${NC} $1"
}

print_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Check if .env file exists
if [ ! -f ".env" ]; then
    print_error ".env file not found!"
    print_status "Please copy env.example to .env and configure your settings"
    exit 1
fi

# Load environment variables
source .env

echo "📊 Configuration Analysis:"
echo "-------------------------"

# Check data collection status
print_status "Checking data collection status..."
if curl -s "http://localhost:8080/health" > /dev/null 2>&1; then
    DATA_COUNT=$(curl -s "http://localhost:8080/prices/SOL%2FUSDC/history?hours=24" | jq -r '.data | length' 2>/dev/null || echo "0")
    print_success "Database service is running"
    print_status "Data points collected: $DATA_COUNT / $MIN_DATA_POINTS"
    
    if [ "$DATA_COUNT" -lt "$MIN_DATA_POINTS" ]; then
        print_warning "Insufficient data points for trading"
        let REMAINING=$MIN_DATA_POINTS-$DATA_COUNT
        let MINUTES_REMAINING=$REMAINING*30/60
        print_status "Need $REMAINING more points (~$MINUTES_REMAINING minutes)"
    else
        print_success "Sufficient data points available"
    fi
else
    print_error "Database service not responding"
fi

echo ""

# Check trading configuration
echo "🎯 Trading Configuration:"
echo "------------------------"

# Check confidence threshold
if [ -n "$MIN_CONFIDENCE_THRESHOLD" ]; then
    CONFIDENCE_PCT=$(echo "$MIN_CONFIDENCE_THRESHOLD * 100" | bc -l 2>/dev/null || echo "70")
    print_status "Minimum confidence threshold: ${CONFIDENCE_PCT}%"
    
    if (( $(echo "$MIN_CONFIDENCE_THRESHOLD > 0.6" | bc -l) )); then
        print_warning "High confidence threshold - may prevent trades"
        print_status "Consider lowering to 0.4-0.5 for more active trading"
    fi
else
    print_warning "MIN_CONFIDENCE_THRESHOLD not set (defaults to 70%)"
fi

# Check trading execution
if [ -n "$ENABLE_TRADING_EXECUTION" ]; then
    if [ "$ENABLE_TRADING_EXECUTION" = "true" ]; then
        print_success "Trading execution: ENABLED (real trades)"
    else
        print_status "Trading execution: DISABLED (paper trading only)"
    fi
else
    print_warning "ENABLE_TRADING_EXECUTION not set (defaults to false)"
fi

# Check position size
if [ -n "$POSITION_SIZE_PERCENTAGE" ]; then
    POSITION_PCT=$(echo "$POSITION_SIZE_PERCENTAGE * 100" | bc -l 2>/dev/null || echo "90")
    print_status "Position size: ${POSITION_PCT}% of balance"
else
    print_warning "POSITION_SIZE_PERCENTAGE not set (defaults to 90%)"
fi

echo ""

# Check recent trading signals
echo "📈 Recent Trading Activity:"
echo "---------------------------"

# Check if trading logic is running
if pgrep -f "trading-logic" > /dev/null; then
    print_success "Trading logic service is running"
    
    # Check recent logs for signals
    if [ -f "logs/trading_logic.log" ]; then
        echo ""
        print_status "Recent trading signals (last 20 lines):"
        echo "----------------------------------------"
        tail -20 logs/trading_logic.log | grep -E "(BUY|SELL|HOLD|SIGNAL|confidence)" || echo "No recent signals found"
    else
        print_warning "No trading logic log file found"
    fi
else
    print_error "Trading logic service is not running"
    print_status "Start it with: ./start_all_screen.sh"
fi

echo ""

# Check price feed status
echo "📡 Price Feed Status:"
echo "--------------------"

if curl -s "http://localhost:8081/health" > /dev/null 2>&1; then
    print_success "Price feed service is running"
    
    # Get latest price
    LATEST_PRICE=$(curl -s "http://localhost:8080/prices/SOL%2FUSDC/latest" | jq -r '.data.price' 2>/dev/null || echo "N/A")
    if [ "$LATEST_PRICE" != "null" ] && [ "$LATEST_PRICE" != "N/A" ]; then
        print_status "Latest SOL/USDC price: \$$LATEST_PRICE"
    fi
else
    print_error "Price feed service not responding"
fi

echo ""

# Check screen sessions
echo "📺 Screen Sessions:"
echo "------------------"

if command -v screen &> /dev/null; then
    TIRADE_SESSIONS=$(screen -list 2>/dev/null | grep "tirade-" || echo "No tirade sessions found")
    if echo "$TIRADE_SESSIONS" | grep -q "tirade-"; then
        print_success "Tirade screen sessions found:"
        echo "$TIRADE_SESSIONS"
    else
        print_warning "No tirade screen sessions running"
        print_status "Start services with: ./start_all_screen.sh"
    fi
else
    print_warning "Screen not installed"
fi

echo ""

# Recommendations
echo "💡 Recommendations:"
echo "------------------"

if [ "$DATA_COUNT" -lt "$MIN_DATA_POINTS" ]; then
    print_status "1. Wait for more data collection or reduce MIN_DATA_POINTS to 100"
fi

if [ -z "$MIN_CONFIDENCE_THRESHOLD" ] || (( $(echo "$MIN_CONFIDENCE_THRESHOLD > 0.6" | bc -l 2>/dev/null || echo "1") )); then
    print_status "2. Lower MIN_CONFIDENCE_THRESHOLD to 0.4-0.5 for more active trading"
fi

if [ "$ENABLE_TRADING_EXECUTION" != "true" ]; then
    print_status "3. Set ENABLE_TRADING_EXECUTION=true for real trading (currently paper trading)"
fi

print_status "4. Check logs with: ./manage_screen.sh logs trading"
print_status "5. Monitor real-time with: ./manage_screen.sh monitor"

echo ""
echo "🔧 Quick Fix Commands:"
echo "---------------------"
echo "  # Lower confidence threshold for more trades:"
echo "  echo 'MIN_CONFIDENCE_THRESHOLD=0.4' >> .env"
echo ""
echo "  # Reduce data requirements:"
echo "  echo 'MIN_DATA_POINTS=100' >> .env"
echo ""
echo "  # Restart services after changes:"
echo "  ./stop_all_screen.sh && ./start_all_screen.sh" 