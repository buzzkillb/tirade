#!/bin/bash

# Create data directory if it doesn't exist
mkdir -p data

# Create an empty SQLite database file
touch data/trading_bot.db

# Set proper permissions
chmod 644 data/trading_bot.db

echo "Database file initialized: data/trading_bot.db"
echo "You can now run the database service with:"
echo "cd database-service && DATABASE_URL=\"sqlite:../data/trading_bot.db\" cargo run" 