#!/bin/bash

echo "🔍 Testing Multiwallet Configuration..."
echo ""

# Check if .env file exists
if [ ! -f "../.env" ]; then
    echo "❌ .env file not found!"
    exit 1
fi

# Source the .env file
source ../.env

echo "📊 Current Configuration:"
echo ""

# Check multiwallet configuration
if [ ! -z "$SOLANA_PRIVATE_KEYS" ]; then
    echo "✅ SOLANA_PRIVATE_KEYS found"
    # Count the number of keys (rough estimate by counting commas + 1)
    key_count=$(echo "$SOLANA_PRIVATE_KEYS" | grep -o ',' | wc -l)
    key_count=$((key_count + 1))
    echo "   - Estimated wallet count: $key_count"
else
    echo "⚠️  SOLANA_PRIVATE_KEYS not found"
    if [ ! -z "$SOLANA_PRIVATE_KEY" ]; then
        echo "✅ SOLANA_PRIVATE_KEY found (single wallet fallback)"
        echo "   - Wallet count: 1"
    else
        echo "❌ No wallet configuration found!"
    fi
fi

if [ ! -z "$WALLET_NAMES" ]; then
    echo "✅ WALLET_NAMES configured: $WALLET_NAMES"
else
    echo "⚠️  WALLET_NAMES not set (will auto-generate)"
fi

echo ""
echo "🎯 Trading Settings:"
echo "   - Trading Execution: ${ENABLE_TRADING_EXECUTION:-false}"
echo "   - Position Size: ${POSITION_SIZE_PERCENTAGE:-0.9}"
echo "   - Min Confidence: ${MIN_CONFIDENCE_THRESHOLD:-0.35}"
echo "   - Trading Pair: ${TRADING_PAIR:-SOL/USDC}"

echo ""
echo "🏦 Multiwallet Status:"
if [ ! -z "$SOLANA_PRIVATE_KEYS" ]; then
    echo "✅ Multiwallet configuration detected!"
    echo "   - The system will use multiple wallets for trading"
    echo "   - Each wallet can maintain independent positions"
    echo "   - Risk is distributed across wallets"
else
    echo "⚠️  Single wallet configuration"
    echo "   - The system will work but with only one wallet"
    echo "   - To enable multiwallet, add SOLANA_PRIVATE_KEYS to .env"
fi

echo ""
echo "💡 To add more wallets:"
echo "   1. Generate new keypairs: solana-keygen new --outfile wallet2.json"
echo "   2. Update .env with: SOLANA_PRIVATE_KEYS=[\"key1\", \"key2\", \"key3\"]"
echo "   3. Optionally add: WALLET_NAMES=[\"Wallet1\", \"Wallet2\", \"Wallet3\"]"