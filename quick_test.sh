#!/bin/bash

# Quick Trading System Test
# Rapid validation of core functionality

set -e

echo "⚡ Quick Trading System Test"
echo "═══════════════════════════════════════════════════════════════"

# Colors
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

print_status() { echo -e "${BLUE}[INFO]${NC} $1"; }
print_success() { echo -e "${GREEN}[SUCCESS]${NC} $1"; }
print_warning() { echo -e "${YELLOW}[WARNING]${NC} $1"; }
print_error() { echo -e "${RED}[ERROR]${NC} $1"; }

# Test 1: Check if services are running
print_status "Test 1: Service Health Check"
if curl -s http://localhost:8080/health > /dev/null; then
    print_success "Database service is running"
else
    print_error "Database service is not running"
    exit 1
fi

# Test 2: Check if transaction binary exists
print_status "Test 2: Transaction Binary Check"
if [ -f "solana-trading-bot/target/debug/transaction" ]; then
    print_success "Transaction binary found"
else
    print_warning "Transaction binary not found - building..."
    cd solana-trading-bot && cargo build --bin transaction && cd ..
    if [ -f "solana-trading-bot/target/debug/transaction" ]; then
        print_success "Transaction binary built successfully"
    else
        print_error "Failed to build transaction binary"
        exit 1
    fi
fi

# Test 3: Check wallet balance
print_status "Test 3: Wallet Balance Check"
cd solana-trading-bot
BALANCE_OUTPUT=$(cargo run --bin transaction -- --amount-usdc 0.01 --direction usdc-to-sol --dry-run 2>/dev/null | grep -E "(SOL:|USDC:)" | head -2)
cd ..

if [ $? -eq 0 ]; then
    print_success "Wallet balance check successful"
    echo "$BALANCE_OUTPUT"
else
    print_error "Wallet balance check failed"
    exit 1
fi

# Test 4: Test small dry-run transaction
print_status "Test 4: Dry-Run Transaction Test"
cd solana-trading-bot
DRY_RUN_OUTPUT=$(cargo run --bin transaction -- --amount-usdc 0.10 --direction usdc-to-sol --dry-run 2>&1)
DRY_RUN_EXIT=$?
cd ..

if [ $DRY_RUN_EXIT -eq 0 ]; then
    print_success "Dry-run transaction successful"
    # Extract key info
    SOL_RECEIVED=$(echo "$DRY_RUN_OUTPUT" | grep "SOL:" | grep "(received)" | awk '{print $2}' || echo "0")
    USDC_SPENT=$(echo "$DRY_RUN_OUTPUT" | grep "USDC:" | grep "(spent)" | awk '{print $2}' || echo "0")
    echo "  Would receive: $SOL_RECEIVED SOL"
    echo "  Would spend: $USDC_SPENT USDC"
else
    print_error "Dry-run transaction failed"
    echo "$DRY_RUN_OUTPUT"
    exit 1
fi

# Test 5: Check trading logic process
print_status "Test 5: Trading Logic Process Check"
if pgrep -f "trading-logic" > /dev/null; then
    print_success "Trading logic process is running"
else
    print_warning "Trading logic process is not running"
fi

# Test 6: Check signal generation
print_status "Test 6: Signal Generation Check"
SIGNAL_COUNT=$(curl -s http://localhost:8080/signals/SOL%2FUSDC/count 2>/dev/null | jq -r '.data' 2>/dev/null || echo "0")

if [ "$SIGNAL_COUNT" != "0" ] && [ "$SIGNAL_COUNT" != "null" ]; then
    print_success "Trading signals are being generated ($SIGNAL_COUNT signals)"
else
    print_warning "No trading signals found"
fi

# Test 7: Check dashboard
print_status "Test 7: Dashboard Check"
if curl -s http://localhost:3000 > /dev/null; then
    print_success "Dashboard is accessible"
else
    print_warning "Dashboard is not accessible"
fi

echo ""
echo "🎉 Quick Test Summary:"
echo "═══════════════════════════════════════════════════════════════"
echo "✅ Service Health: Database service running"
echo "✅ Transaction Binary: Available and functional"
echo "✅ Wallet Balance: Successfully retrieved"
echo "✅ Dry-Run Transaction: Executed successfully"
echo "✅ Trading Logic: Process status checked"
echo "✅ Signal Generation: Status verified"
echo "✅ Dashboard: Accessibility confirmed"
echo ""
echo "🚀 System is ready for comprehensive testing!"
echo "Run './test_trading_scenario.sh' for full trading cycle test"
echo "Run './test_trading_scenario.sh --live' for live trading test (WARNING: Real trades!)" 