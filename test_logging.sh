#!/bin/bash

# Test script for the logging service
# This script sends sample logs to test the logging service functionality

echo "🧪 Testing Logging Service..."

# Wait for logging service to be ready
echo "⏳ Waiting for logging service to be ready..."
for i in {1..30}; do
    if curl -s "http://localhost:8083" > /dev/null 2>&1; then
        echo "✅ Logging service is ready!"
        break
    fi
    echo -n "."
    sleep 1
done

# Send test logs
echo "📤 Sending test logs..."

# Test log 1 - Info level
curl -X POST "http://localhost:8083/logs" \
  -H "Content-Type: application/json" \
  -d '{
    "timestamp": "'$(date -u +"%Y-%m-%dT%H:%M:%SZ")'",
    "level": "INFO",
    "service": "test-service",
    "message": "🟢 BUY signal detected - no current position, executing trade..."
  }'

echo ""

# Test log 2 - Warning level with sensitive data
curl -X POST "http://localhost:8083/logs" \
  -H "Content-Type: application/json" \
  -d '{
    "timestamp": "'$(date -u +"%Y-%m-%dT%H:%M:%SZ")'",
    "level": "WARN",
    "service": "trading-logic",
    "message": "⚠️  Wallet address 1A1zP1eP5QGefi2DMPTfTL5SLmv7DivfNa detected in transaction"
  }'

echo ""

# Test log 3 - Error level
curl -X POST "http://localhost:8083/logs" \
  -H "Content-Type: application/json" \
  -d '{
    "timestamp": "'$(date -u +"%Y-%m-%dT%H:%M:%SZ")'",
    "level": "ERROR",
    "service": "database-service",
    "message": "❌ Failed to connect to database: sqlite:///path/to/database.db"
  }'

echo ""

# Test log 4 - Debug level
curl -X POST "http://localhost:8083/logs" \
  -H "Content-Type: application/json" \
  -d '{
    "timestamp": "'$(date -u +"%Y-%m-%dT%H:%M:%SZ")'",
    "level": "DEBUG",
    "service": "price-feed",
    "message": "🔍 Fetching price data from Pyth Network..."
  }'

echo ""

# Test log 5 - Info level with API key
curl -X POST "http://localhost:8083/logs" \
  -H "Content-Type: application/json" \
  -d '{
    "timestamp": "'$(date -u +"%Y-%m-%dT%H:%M:%SZ")'",
    "level": "INFO",
    "service": "trading-executor",
    "message": "🔗 Connecting to Solana RPC: https://api.mainnet-beta.solana.com with api_key=secret123"
  }'

echo ""

echo "✅ Test logs sent successfully!"
echo "📊 Check the dashboard at http://localhost:3000 to see the logs in real-time"
echo "🔗 Logging service status: http://localhost:8083" 