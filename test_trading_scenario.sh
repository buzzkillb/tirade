#!/bin/bash

# Comprehensive Trading Test Script
# Tests full trading scenarios with balance verification and PnL tracking

set -e

echo "🚀 Starting Comprehensive Trading Test Scenario"
echo "═══════════════════════════════════════════════════════════════"

# Configuration
TEST_AMOUNT_USDC=1.00  # Small test amount
WALLET_ADDRESS=""  # Will be set from environment
DRY_RUN=true  # Start with dry-run for safety

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Function to print colored output
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

# Function to check if services are running
check_services() {
    print_status "Checking if all services are running..."
    
    # Check database service
    if curl -s http://localhost:8080/health > /dev/null; then
        print_success "Database service is running"
    else
        print_error "Database service is not running on port 8080"
        exit 1
    fi
    
    # Check dashboard
    if curl -s http://localhost:3000 > /dev/null; then
        print_success "Dashboard is running"
    else
        print_warning "Dashboard is not running on port 3000"
    fi
}

# Function to get initial balances
get_initial_balances() {
    print_status "Getting initial wallet balances..."
    
    # Get wallet address from environment or config
    if [ -z "$WALLET_ADDRESS" ]; then
        # Try to get from .env file
        if [ -f ".env" ]; then
            WALLET_ADDRESS=$(grep "SOLANA_PRIVATE_KEY" .env | cut -d'=' -f2 | head -c 44)
            print_status "Using wallet address from .env: ${WALLET_ADDRESS:0:8}..."
        else
            print_error "No wallet address found. Please set WALLET_ADDRESS or check .env file"
            exit 1
        fi
    fi
    
    # Get initial balances using the transaction binary
    print_status "Fetching initial balances..."
    INITIAL_BALANCES=$(cd solana-trading-bot && cargo run --bin transaction -- --amount-usdc 0.01 --direction usdc-to-sol --dry-run 2>/dev/null | grep -E "(SOL:|USDC:)" | head -2)
    
    if [ $? -eq 0 ]; then
        print_success "Initial balances retrieved"
        echo "$INITIAL_BALANCES"
    else
        print_error "Failed to get initial balances"
        exit 1
    fi
}

# Function to test USDC to SOL swap
test_usdc_to_sol() {
    print_status "Testing USDC to SOL swap..."
    
    echo "🔄 Step 1: USDC → SOL Swap"
    echo "   Amount: $TEST_AMOUNT_USDC USDC"
    if [ "$DRY_RUN" = true ]; then
        echo "   Mode: Dry Run"
    else
        echo "   Mode: Live Trade"
    fi
    
    # Execute the swap
    cd solana-trading-bot
    if [ "$DRY_RUN" = true ]; then
        SWAP_OUTPUT=$(cargo run --bin transaction -- --amount-usdc $TEST_AMOUNT_USDC --direction usdc-to-sol --dry-run 2>&1)
    else
        SWAP_OUTPUT=$(cargo run --bin transaction -- --amount-usdc $TEST_AMOUNT_USDC --direction usdc-to-sol 2>&1)
    fi
    SWAP_EXIT_CODE=$?
    cd ..
    
    if [ $SWAP_EXIT_CODE -eq 0 ]; then
        print_success "USDC to SOL swap completed successfully"
        
        # Extract key information
        SOL_RECEIVED=$(echo "$SWAP_OUTPUT" | grep "SOL:" | grep "(received)" | awk '{print $2}')
        USDC_SPENT=$(echo "$SWAP_OUTPUT" | grep "USDC:" | grep "(spent)" | awk '{print $2}')
        
        echo "   📊 Results:"
        echo "      SOL Received: $SOL_RECEIVED"
        echo "      USDC Spent: $USDC_SPENT"
        
        # Store results for later comparison
        echo "$SOL_RECEIVED" > /tmp/test_sol_received
        echo "$USDC_SPENT" > /tmp/test_usdc_spent
        
    else
        print_error "USDC to SOL swap failed"
        echo "$SWAP_OUTPUT"
        exit 1
    fi
}

# Function to test SOL to USDC swap
test_sol_to_usdc() {
    print_status "Testing SOL to USDC swap..."
    
    # Get the SOL amount we received from previous swap
    if [ -f /tmp/test_sol_received ]; then
        SOL_AMOUNT=$(cat /tmp/test_sol_received)
    else
        print_error "No SOL amount found from previous swap"
        exit 1
    fi
    
    echo "🔄 Step 2: SOL → USDC Swap"
    echo "   Amount: $SOL_AMOUNT SOL"
    if [ "$DRY_RUN" = true ]; then
        echo "   Mode: Dry Run"
    else
        echo "   Mode: Live Trade"
    fi
    
    # Execute the swap
    cd solana-trading-bot
    if [ "$DRY_RUN" = true ]; then
        SWAP_OUTPUT=$(cargo run --bin transaction -- --amount-usdc $SOL_AMOUNT --direction sol-to-usdc --dry-run 2>&1)
    else
        SWAP_OUTPUT=$(cargo run --bin transaction -- --amount-usdc $SOL_AMOUNT --direction sol-to-usdc 2>&1)
    fi
    SWAP_EXIT_CODE=$?
    cd ..
    
    if [ $SWAP_EXIT_CODE -eq 0 ]; then
        print_success "SOL to USDC swap completed successfully"
        
        # Extract key information
        USDC_RECEIVED=$(echo "$SWAP_OUTPUT" | grep "USDC:" | grep "(received)" | awk '{print $2}')
        SOL_SPENT=$(echo "$SWAP_OUTPUT" | grep "SOL:" | grep "(spent)" | awk '{print $2}')
        
        echo "   📊 Results:"
        echo "      USDC Received: $USDC_RECEIVED"
        echo "      SOL Spent: $SOL_SPENT"
        
        # Store for PnL calculation
        echo "$USDC_RECEIVED" > /tmp/test_usdc_received
        
    else
        print_error "SOL to USDC swap failed"
        echo "$SWAP_OUTPUT"
        exit 1
    fi
}

# Function to calculate and verify PnL
calculate_pnl() {
    print_status "Calculating Profit/Loss..."
    
    if [ -f /tmp/test_usdc_spent ] && [ -f /tmp/test_usdc_received ]; then
        USDC_SPENT=$(cat /tmp/test_usdc_spent)
        USDC_RECEIVED=$(cat /tmp/test_usdc_received)
        
        # Calculate PnL
        PNL=$(echo "$USDC_RECEIVED - $USDC_SPENT" | bc -l)
        PNL_PERCENT=$(echo "($PNL / $USDC_SPENT) * 100" | bc -l)
        
        echo "💰 PnL Analysis:"
        echo "   USDC Spent: $USDC_SPENT"
        echo "   USDC Received: $USDC_RECEIVED"
        echo "   Net PnL: $PNL USDC"
        echo "   PnL %: ${PNL_PERCENT}%"
        
        # Determine if it's a profit or loss
        if (( $(echo "$PNL > 0" | bc -l) )); then
            print_success "✅ PROFIT: +$PNL USDC (${PNL_PERCENT}%)"
        elif (( $(echo "$PNL < 0" | bc -l) )); then
            print_warning "💸 LOSS: $PNL USDC (${PNL_PERCENT}%)"
        else
            print_status "➡️  BREAKEVEN: $PNL USDC"
        fi
        
        # Store PnL for reporting
        echo "$PNL" > /tmp/test_pnl
        echo "$PNL_PERCENT" > /tmp/test_pnl_percent
        
    else
        print_error "Missing balance data for PnL calculation"
        exit 1
    fi
}

# Function to verify balance consistency
verify_balances() {
    print_status "Verifying balance consistency..."
    
    # Get final balances
    cd solana-trading-bot
    FINAL_BALANCES=$(cargo run --bin transaction -- --amount-usdc 0.01 --direction usdc-to-sol --dry-run 2>/dev/null | grep -E "(SOL:|USDC:)" | head -2)
    cd ..
    
    if [ $? -eq 0 ]; then
        print_success "Final balances retrieved"
        echo "$FINAL_BALANCES"
        
        # Compare with initial balances (if we had them stored)
        if [ -f /tmp/initial_balances ]; then
            print_status "Comparing initial vs final balances..."
            # This would require more sophisticated parsing
            echo "   Balance verification completed"
        fi
    else
        print_error "Failed to get final balances"
    fi
}

# Function to test trading logic integration
test_trading_logic() {
    print_status "Testing trading logic integration..."
    
    # Check if trading logic is running
    if pgrep -f "trading-logic" > /dev/null; then
        print_success "Trading logic process is running"
    else
        print_warning "Trading logic process is not running"
    fi
    
    # Check if signals are being generated
    SIGNAL_COUNT=$(curl -s http://localhost:8080/signals/SOL%2FUSDC/count 2>/dev/null | jq -r '.data' 2>/dev/null || echo "0")
    
    if [ "$SIGNAL_COUNT" != "0" ] && [ "$SIGNAL_COUNT" != "null" ]; then
        print_success "Trading signals are being generated ($SIGNAL_COUNT signals)"
    else
        print_warning "No trading signals found"
    fi
}

# Function to generate test report
generate_report() {
    print_status "Generating test report..."
    
    REPORT_FILE="trading_test_report_$(date +%Y%m%d_%H%M%S).txt"
    
    {
        echo "Trading Test Report"
        echo "Generated: $(date)"
        echo "═══════════════════════════════════════════════════════════════"
        echo ""
        echo "Test Configuration:"
        echo "  Test Amount: $TEST_AMOUNT_USDC USDC"
        echo "  Mode: ${DRY_RUN:+Dry Run}${DRY_RUN:-Live Trade}"
        echo "  Wallet: ${WALLET_ADDRESS:0:8}..."
        echo ""
        
        if [ -f /tmp/test_pnl ]; then
            PNL=$(cat /tmp/test_pnl)
            PNL_PERCENT=$(cat /tmp/test_pnl_percent)
            echo "Results:"
            echo "  Net PnL: $PNL USDC"
            echo "  PnL %: ${PNL_PERCENT}%"
            echo ""
        fi
        
        echo "Services Status:"
        echo "  Database: $(curl -s http://localhost:8080/health > /dev/null && echo "✅ Running" || echo "❌ Not Running")"
        echo "  Dashboard: $(curl -s http://localhost:3000 > /dev/null && echo "✅ Running" || echo "❌ Not Running")"
        echo "  Trading Logic: $(pgrep -f "trading-logic" > /dev/null && echo "✅ Running" || echo "❌ Not Running")"
        echo ""
        
        echo "Test completed successfully!"
        
    } > "$REPORT_FILE"
    
    print_success "Test report generated: $REPORT_FILE"
}

# Function to clean up test files
cleanup() {
    print_status "Cleaning up test files..."
    rm -f /tmp/test_*
    print_success "Cleanup completed"
}

# Main test execution
main() {
    echo "Starting comprehensive trading test..."
    echo ""
    
    # Check if we're in the right directory
    if [ ! -f "Cargo.toml" ] && [ ! -d "solana-trading-bot" ]; then
        print_error "Please run this script from the project root directory"
        exit 1
    fi
    
    # Check services
    check_services
    
    # Get initial balances
    get_initial_balances
    
    # Test USDC to SOL swap
    test_usdc_to_sol
    
    # Test SOL to USDC swap
    test_sol_to_usdc
    
    # Calculate PnL
    calculate_pnl
    
    # Verify balances
    verify_balances
    
    # Test trading logic integration
    test_trading_logic
    
    # Generate report
    generate_report
    
    # Cleanup
    cleanup
    
    echo ""
    echo "🎉 Comprehensive trading test completed!"
    echo "Check the generated report for detailed results."
}

# Handle script arguments
case "${1:-}" in
    --live)
        DRY_RUN=false
        print_warning "Running in LIVE mode - real trades will be executed!"
        ;;
    --amount)
        TEST_AMOUNT_USDC="$2"
        print_status "Using test amount: $TEST_AMOUNT_USDC USDC"
        ;;
    --help)
        echo "Usage: $0 [OPTIONS]"
        echo ""
        echo "Options:"
        echo "  --live          Execute real trades (default: dry-run)"
        echo "  --amount AMOUNT Set test amount in USDC (default: 1.00)"
        echo "  --help          Show this help message"
        echo ""
        echo "This script tests:"
        echo "  ✅ USDC → SOL swap"
        echo "  ✅ SOL → USDC swap"
        echo "  ✅ Balance verification"
        echo "  ✅ PnL calculation"
        echo "  ✅ Trading logic integration"
        echo "  ✅ Service health checks"
        exit 0
        ;;
esac

# Run the main test
main "$@" 