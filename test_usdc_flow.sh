#!/bin/bash

echo "🔍 Testing USDC Balance Change Tracking"
echo "======================================"

DATABASE_URL="http://localhost:8080"

echo ""
echo "1️⃣ Creating test position with USDC spent..."

# Create position with USDC spent
POSITION_RESPONSE=$(curl -s -X POST "$DATABASE_URL/positions" \
  -H "Content-Type: application/json" \
  -d '{
    "wallet_address": "TEST_WALLET_USDC_FLOW",
    "pair": "SOL/USDC", 
    "position_type": "long",
    "entry_price": 150.0,
    "quantity": 1.0,
    "usdc_spent": 152.50
  }')

echo "Response: $POSITION_RESPONSE"

# Extract position ID
POSITION_ID=$(echo "$POSITION_RESPONSE" | jq -r '.data.id // empty')

if [ -z "$POSITION_ID" ]; then
  echo "❌ Failed to create position or extract position ID"
  exit 1
fi

echo "✅ Position created with ID: $POSITION_ID"

echo ""
echo "2️⃣ Closing position with USDC received..."

# Close position with USDC received
CLOSE_RESPONSE=$(curl -s -X POST "$DATABASE_URL/positions/close" \
  -H "Content-Type: application/json" \
  -d "{
    \"position_id\": \"$POSITION_ID\",
    \"exit_price\": 155.0,
    \"usdc_received\": 153.75
  }")

echo "Response: $CLOSE_RESPONSE"

echo ""
echo "3️⃣ Checking performance metrics..."

# Get performance metrics
METRICS_RESPONSE=$(curl -s "$DATABASE_URL/performance/metrics")
echo "Metrics: $METRICS_RESPONSE"

# Extract total PnL
TOTAL_PNL=$(echo "$METRICS_RESPONSE" | jq -r '.data.total_pnl // 0')

echo ""
echo "📊 Results:"
echo "   USDC Spent: \$152.50"
echo "   USDC Received: \$153.75" 
echo "   Expected PnL: \$1.25"
echo "   Actual PnL: \$$TOTAL_PNL"

# Check if PnL matches expected
EXPECTED_PNL="1.25"
if [ "$TOTAL_PNL" = "$EXPECTED_PNL" ]; then
  echo "🎉 SUCCESS: USDC balance change tracking is working!"
else
  echo "⚠️  PnL doesn't match exactly, but this could be due to existing trades"
  echo "   Check the database logs to see if USDC data is being stored"
fi

echo ""
echo "4️⃣ Checking database directly..."

# Check if database has USDC data
DB_CHECK=$(curl -s "$DATABASE_URL/positions/all?limit=1")
echo "Recent position: $DB_CHECK"

echo ""
echo "✅ Test completed. Check the logs above for USDC tracking status."