#!/bin/bash
sqlite3 data/trading_bot.db "\
.headers on
.mode column
SELECT \
  COUNT(*) AS num_trades,\
  SUM(pnl) AS total_pnl,\
  AVG(pnl) AS avg_pnl,\
  SUM(pnl_percent) AS total_pnl_percent,\
  AVG(pnl_percent) AS avg_pnl_percent\
FROM positions\
WHERE status = 'closed';\
" 