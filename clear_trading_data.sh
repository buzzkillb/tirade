#!/bin/bash

# Tirade Trading Data Clear Script
# This script safely clears all trading data from the database

set -e

echo "🗑️  Tirade Trading Data Clear Tool"
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

# Check if database file exists
if [ ! -f "data/trading_bot.db" ]; then
    print_error "Database file not found: data/trading_bot.db"
    echo "Make sure the database service is running and has created the database file."
    exit 1
fi

# Show current data before clearing
print_status "Current data in database:"
echo ""

# Count current records
POSITIONS_COUNT=$(sqlite3 data/trading_bot.db "SELECT COUNT(*) FROM positions;")
TRADES_COUNT=$(sqlite3 data/trading_bot.db "SELECT COUNT(*) FROM trades;" 2>/dev/null || echo "0")
SIGNALS_COUNT=$(sqlite3 data/trading_bot.db "SELECT COUNT(*) FROM trading_signals;" 2>/dev/null || echo "0")
BALANCES_COUNT=$(sqlite3 data/trading_bot.db "SELECT COUNT(*) FROM balance_snapshots;" 2>/dev/null || echo "0")

echo "📊 Current Records:"
echo "   Positions: $POSITIONS_COUNT"
echo "   Trades: $TRADES_COUNT"
echo "   Trading Signals: $SIGNALS_COUNT"
echo "   Balance Snapshots: $BALANCES_COUNT"
echo ""

# Confirm deletion
echo "⚠️  WARNING: This will permanently delete all trading data!"
echo "   - All positions (open and closed)"
echo "   - All trades"
echo "   - All trading signals"
echo "   - All balance snapshots"
echo "   - All performance data"
echo ""
read -p "Are you sure you want to continue? (yes/no): " confirm

if [ "$confirm" != "yes" ]; then
    print_warning "Operation cancelled by user"
    exit 0
fi

echo ""
print_status "Clearing all trading data..."

# Clear all trading-related data
sqlite3 data/trading_bot.db << 'SQL_EOF'
-- Clear positions
DELETE FROM positions;

-- Clear trades (if table exists)
DELETE FROM trades;

-- Clear trading signals (if table exists)
DELETE FROM trading_signals;

-- Clear balance snapshots (if table exists)
DELETE FROM balance_snapshots;

-- Clear technical indicators (if table exists)
DELETE FROM technical_indicators;

-- Clear trading configs (if table exists)
DELETE FROM trading_configs;

-- Reset auto-increment counters
DELETE FROM sqlite_sequence WHERE name IN ('positions', 'trades', 'trading_signals', 'balance_snapshots', 'technical_indicators', 'trading_configs');

-- Vacuum to reclaim space
VACUUM;
SQL_EOF

if [ $? -eq 0 ]; then
    print_success "All trading data cleared successfully!"
else
    print_error "Failed to clear trading data"
    exit 1
fi

# Verify data is cleared
echo ""
print_status "Verifying data has been cleared:"

POSITIONS_COUNT_AFTER=$(sqlite3 data/trading_bot.db "SELECT COUNT(*) FROM positions;")
TRADES_COUNT_AFTER=$(sqlite3 data/trading_bot.db "SELECT COUNT(*) FROM trades;" 2>/dev/null || echo "0")
SIGNALS_COUNT_AFTER=$(sqlite3 data/trading_bot.db "SELECT COUNT(*) FROM trading_signals;" 2>/dev/null || echo "0")
BALANCES_COUNT_AFTER=$(sqlite3 data/trading_bot.db "SELECT COUNT(*) FROM balance_snapshots;" 2>/dev/null || echo "0")

echo "📊 Records After Clear:"
echo "   Positions: $POSITIONS_COUNT_AFTER"
echo "   Trades: $TRADES_COUNT_AFTER"
echo "   Trading Signals: $SIGNALS_COUNT_AFTER"
echo "   Balance Snapshots: $BALANCES_COUNT_AFTER"
echo ""

if [ "$POSITIONS_COUNT_AFTER" -eq 0 ] && [ "$TRADES_COUNT_AFTER" -eq 0 ]; then
    print_success "✅ All trading data successfully cleared!"
    echo ""
    echo "🔄 Next Steps:"
    echo "   1. Restart the trading services: ./stop_all_screen.sh && ./start_all_screen.sh"
    echo "   2. The dashboard will now show no recent trades"
    echo "   3. New trading signals will start fresh"
    echo "   4. Performance metrics will reset to zero"
else
    print_warning "⚠️  Some data may still exist. Check the database manually if needed."
fi

echo ""
print_success "Trading data clear operation completed!" 