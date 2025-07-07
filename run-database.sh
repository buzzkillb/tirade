#!/bin/bash

# Initialize database if needed
if [ ! -f "data/trading_bot.db" ]; then
    echo "Initializing database..."
    ./init-database.sh
fi

# Kill any existing database service processes
pkill -f database-service

# Wait a moment for processes to stop
sleep 2

# Start the database service with correct environment variables
cd database-service
DATABASE_URL="sqlite:../data/trading_bot.db" cargo run 