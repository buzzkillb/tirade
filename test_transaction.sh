#!/bin/bash

# Tirade Transaction Test Script
# Tests different transaction parameters to identify issues

set -e

echo "🧪 Tirade Transaction Test Tool"
echo "==============================="
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

# Load environment variables
source .env

# Function to test transaction
test_transaction() {
    local amount=$1
    local slippage=$2
    local description=$3
    
    print_status "Testing: $description"
    print_status "Amount: $amount USDC, Slippage: $slippage bps"
    
    cd solana-trading-bot
    
    # First try dry run
    print_status "Running dry run..."
    DRY_RUN_OUTPUT=$(SOLANA_RPC_URL="$SOLANA_RPC_URL" SOLANA_PRIVATE_KEY="$SOLANA_PRIVATE_KEY" cargo run --bin transaction -- --amount-usdc "$amount" --direction usdc-to-sol --slippage-bps "$slippage" --dry-run 2>&1)
    
    if [ $? -eq 0 ]; then
        print_success "Dry run successful"
        echo "$DRY_RUN_OUTPUT" | tail -10
    else
        print_error "Dry run failed"
        echo "$DRY_RUN_OUTPUT"
        cd ..
        return 1
    fi
    
    # Ask user if they want to try real transaction
    echo ""
    read -p "Do you want to try a real transaction with $amount USDC? (y/N): " -n 1 -r
    echo ""
    
    if [[ $REPLY =~ ^[Yy]$ ]]; then
        print_status "Executing real transaction..."
        REAL_OUTPUT=$(SOLANA_RPC_URL="$SOLANA_RPC_URL" SOLANA_PRIVATE_KEY="$SOLANA_PRIVATE_KEY" cargo run --bin transaction -- --amount-usdc "$amount" --direction usdc-to-sol --slippage-bps "$slippage" 2>&1)
        
        if [ $? -eq 0 ]; then
            print_success "Real transaction successful!"
            echo "$REAL_OUTPUT" | tail -20
        else
            print_error "Real transaction failed"
            echo "$REAL_OUTPUT"
        fi
    fi
    
    cd ..
    echo ""
}

echo "🔍 Testing different transaction parameters..."
echo ""

# Test 1: Small amount with low slippage
test_transaction "1.0" "50" "Small amount (1 USDC) with low slippage (0.5%)"

# Test 2: Small amount with higher slippage
test_transaction "1.0" "100" "Small amount (1 USDC) with higher slippage (1%)"

# Test 3: Medium amount with low slippage
test_transaction "10.0" "50" "Medium amount (10 USDC) with low slippage (0.5%)"

# Test 4: Medium amount with higher slippage
test_transaction "10.0" "100" "Medium amount (10 USDC) with higher slippage (1%)"

echo "✅ All tests completed!"
echo ""
echo "📊 Summary:"
echo "- If dry runs fail: Check balances and RPC connection"
echo "- If dry runs succeed but real transactions fail: Network/API issues"
echo "- If small amounts work but large amounts fail: Insufficient balance or slippage"
echo ""
echo "💡 Next steps:"
echo "1. Check your wallet balances"
echo "2. Try with even smaller amounts (0.1 USDC)"
echo "3. Increase slippage tolerance further (200 bps = 2%)"
echo "4. Check Solana network status" 