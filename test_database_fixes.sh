#!/bin/bash

echo "🧪 Testing Database Fixes"
echo "═══════════════════════════════════════════════════════════════"

# Test 1: Database Health Check
echo "📊 Test 1: Database Health Check"
curl -s http://localhost:8080/health | jq .
echo ""

# Test 2: Create Wallet
echo "📊 Test 2: Create Wallet"
curl -s -X POST http://localhost:8080/wallets \
  -H "Content-Type: application/json" \
  -d '{"address": "test_wallet_123"}' | jq .
echo ""

# Test 3: Create Position
echo "📊 Test 3: Create Position"
curl -s -X POST http://localhost:8080/positions \
  -H "Content-Type: application/json" \
  -d '{
    "wallet_address": "test_wallet_123",
    "pair": "SOL/USDC",
    "position_type": "long",
    "entry_price": 100.0,
    "quantity": 1.0
  }' | jq .
echo ""

# Test 4: Get Open Position
echo "📊 Test 4: Get Open Position"
curl -s "http://localhost:8080/positions/pair/SOL%2FUSDC/open" | jq .
echo ""

# Test 5: Close Position (if position exists)
echo "📊 Test 5: Close Position"
POSITION_ID=$(curl -s "http://localhost:8080/positions/pair/SOL%2FUSDC/open" | jq -r '.data.id // empty')
if [ ! -z "$POSITION_ID" ]; then
    curl -s -X POST http://localhost:8080/positions/close \
      -H "Content-Type: application/json" \
      -d "{
        \"position_id\": \"$POSITION_ID\",
        \"exit_price\": 105.0,
        \"transaction_hash\": null,
        \"fees\": null
      }" | jq .
else
    echo "No open position found to close"
fi
echo ""

echo "✅ Database tests completed!" 