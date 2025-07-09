#!/bin/bash

# Check if database file exists
if [ ! -f "data/trading_bot.db" ]; then
    echo "❌ Database file not found: data/trading_bot.db"
    echo "Make sure the database service is running and has created the database file."
    exit 1
fi

echo "📈 PnL Summary (Closed Positions)"
echo "==============================="

sqlite3 data/trading_bot.db <<SQL_EOF
.headers on
.mode column
SELECT 
  COUNT(*) AS num_trades,
  SUM(pnl) AS total_pnl,
  AVG(pnl) AS avg_pnl,
  SUM(pnl_percent) AS total_pnl_percent,
  AVG(pnl_percent) AS avg_pnl_percent
FROM positions
WHERE status = 'closed';
SQL_EOF 