#!/bin/bash
sqlite3 data/trading_bot.db "\
.headers on
.mode column
SELECT id, pair, position_type, entry_price, exit_price, quantity, pnl, pnl_percent, status, created_at, exit_time \
FROM positions \
WHERE status = 'closed' \
ORDER BY exit_time DESC \
LIMIT 10;\
" 