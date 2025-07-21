#!/bin/bash

echo "🧠 Resetting Neural Network State..."
echo "======================================"

# Stop the trading system first
echo "1. Stopping trading system..."
./stop_all.sh

# Wait a moment for processes to stop
sleep 3

# Clear neural state from database
echo "2. Clearing neural state from database..."
sqlite3 data/trading_bot.db "DELETE FROM neural_state;" 2>/dev/null || echo "No neural_state table found or already empty"

# Drop the table completely
echo "3. Dropping neural_state table..."
sqlite3 data/trading_bot.db "DROP TABLE IF EXISTS neural_state;" 2>/dev/null || echo "Table dropped or didn't exist"

echo "4. Neural network state reset complete!"
echo ""
echo "The neural network will now start fresh with:"
echo "  - Zero predictions (0 total_predictions)"
echo "  - Zero correct predictions (0 correct_predictions)" 
echo "  - Default weights (momentum: 0.3, RSI: 0.4, volatility: 0.3)"
echo "  - Default learning rate (0.01)"
echo ""
echo "5. Restarting trading system..."
./start_all.sh

echo ""
echo "✅ Neural network reset complete! The system will now learn from scratch." 