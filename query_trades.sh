#!/bin/bash

# Check if database file exists
if [ ! -f "data/trading_bot.db" ]; then
    echo "❌ Database file not found: data/trading_bot.db"
    echo "Make sure the database service is running and has created the database file."
    exit 1
fi

echo "📊 Recent Trades (Last 10)"
echo "=========================="

sqlite3 data/trading_bot.db << 'SQL_EOF'
.headers on
.mode column
SELECT id, pair, position_type, entry_price, exit_price, quantity, pnl, pnl_percent, entry_time, exit_time, status
FROM positions 
ORDER BY entry_time DESC 
LIMIT 10;
SQL_EOF 