#!/bin/bash

echo "🧠 Temporarily Disabling Neural Network..."
echo "=========================================="

# Stop the trading system
echo "1. Stopping trading system..."
./stop_all.sh

# Wait for processes to stop
sleep 3

# Create a backup of current .env if it exists
if [ -f .env ]; then
    cp .env .env.backup
    echo "2. Created backup of current .env as .env.backup"
fi

# Add or update NEURAL_ENABLED=false in .env
if [ -f .env ]; then
    # Remove existing NEURAL_ENABLED line if it exists
    sed -i '/^NEURAL_ENABLED=/d' .env
    # Add NEURAL_ENABLED=false
    echo "NEURAL_ENABLED=false" >> .env
else
    echo "NEURAL_ENABLED=false" > .env
fi

echo "3. Set NEURAL_ENABLED=false in .env"
echo ""
echo "4. Restarting trading system without neural network..."
./start_all.sh

echo ""
echo "✅ Neural network disabled! The system will now use only ML strategy."
echo "To re-enable neural network later, run: ./enable_neural.sh" 