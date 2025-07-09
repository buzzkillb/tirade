#!/bin/bash

# Tirade Transaction Diagnostic Script
# This script helps diagnose transaction failures

set -e

echo "🔍 Tirade Transaction Diagnostic Tool"
echo "====================================="
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
    exit 1
fi

# Load environment variables
source .env

# Check if transaction binary exists
if [ ! -f "solana-trading-bot/target/debug/transaction" ]; then
    print_error "Transaction binary not found!"
    print_status "Building transaction binary..."
    cd solana-trading-bot
    cargo build --bin transaction
    cd ..
fi

print_status "Checking wallet balances..."

# Test with a very small amount first (dry run)
print_status "Testing with 0.01 USDC (dry run)..."
cd solana-trading-bot
TRANSACTION_OUTPUT=$(SOLANA_RPC_URL="$SOLANA_RPC_URL" SOLANA_PRIVATE_KEY="$SOLANA_PRIVATE_KEY" cargo run --bin transaction -- --amount-usdc 0.01 --direction usdc-to-sol --slippage-bps 50 --dry-run 2>&1)

if [ $? -eq 0 ]; then
    print_success "Dry run successful"
    echo "$TRANSACTION_OUTPUT"
else
    print_error "Dry run failed"
    echo "$TRANSACTION_OUTPUT"
fi

echo ""
print_status "Testing with 1 USDC (dry run)..."
TRANSACTION_OUTPUT=$(SOLANA_RPC_URL="$SOLANA_RPC_URL" SOLANA_PRIVATE_KEY="$SOLANA_PRIVATE_KEY" cargo run --bin transaction -- --amount-usdc 1.0 --direction usdc-to-sol --slippage-bps 50 --dry-run 2>&1)

if [ $? -eq 0 ]; then
    print_success "1 USDC dry run successful"
    echo "$TRANSACTION_OUTPUT"
else
    print_error "1 USDC dry run failed"
    echo "$TRANSACTION_OUTPUT"
fi

echo ""
print_status "Testing with 10 USDC (dry run)..."
TRANSACTION_OUTPUT=$(SOLANA_RPC_URL="$SOLANA_RPC_URL" SOLANA_PRIVATE_KEY="$SOLANA_PRIVATE_KEY" cargo run --bin transaction -- --amount-usdc 10.0 --direction usdc-to-sol --slippage-bps 50 --dry-run 2>&1)

if [ $? -eq 0 ]; then
    print_success "10 USDC dry run successful"
    echo "$TRANSACTION_OUTPUT"
else
    print_error "10 USDC dry run failed"
    echo "$TRANSACTION_OUTPUT"
fi

cd ..

echo ""
echo "🔧 Troubleshooting Steps:"
echo "1. Check your SOL balance (need at least 0.01 SOL for fees)"
echo "2. Check your USDC balance"
echo "3. Try with a smaller amount first (1-10 USDC)"
echo "4. Increase slippage tolerance (try 100 bps = 1%)"
echo "5. Check if Jupiter API is responding"
echo "6. Verify RPC endpoint is stable"

echo ""
echo "💡 Suggested fixes:"
echo "- Try with smaller amounts first"
echo "- Increase slippage tolerance: --slippage-bps 100"
echo "- Check network congestion"
echo "- Ensure sufficient SOL for transaction fees" 