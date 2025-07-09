#!/bin/bash
sqlite3 data/trading_bot.db "\
.headers on
.mode column
SELECT id, pair, trade_type, entry_price, exit_price, quantity, pnl, pnl_percent, entry_time, exit_time \
FROM trades \
ORDER BY exit_time DESC \
LIMIT 10;\
" 