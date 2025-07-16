# Multiwallet Setup Guide

## Current Status
✅ **Multiwallet code is fully implemented and ready to use!**

## To Add More Wallets:

### Step 1: Generate Additional Wallets
```bash
# Generate new wallet keypairs
solana-keygen new --outfile wallet2.json
solana-keygen new --outfile wallet3.json

# Get the private keys (arrays of numbers)
cat wallet2.json  # Copy the array
cat wallet3.json  # Copy the array
```

### Step 2: Update .env Configuration
```bash
# Add multiple private keys (JSON array format)
SOLANA_PRIVATE_KEYS=[
  "[92,114,64,22,184,252,183,93,106,192,66,65,158,57,75,58,176,208,75,52,228,84,54,4,15,76,220,66,78,41,171,252,165,222,160,254,41,200,214,53,236,101,100,253,175,35,43,162,21,58,209,215,127,93,3,14,55,108,242,102,216,142,253,142]",
  "[YOUR_WALLET_2_PRIVATE_KEY_ARRAY]",
  "[YOUR_WALLET_3_PRIVATE_KEY_ARRAY]"
]

# Optional: Custom wallet names
WALLET_NAMES=["Main_Wallet", "Trading_Wallet_2", "Trading_Wallet_3"]
```

### Step 3: Fund Your Wallets
```bash
# Get wallet addresses
solana address --keypair wallet2.json
solana address --keypair wallet3.json

# Send SOL and USDC to each wallet for trading
```

## How Multiwallet Works

### Trading Strategy:
1. **BUY Signals**: Uses the first available wallet (has USDC, no open position)
2. **SELL Signals**: Closes positions across ALL wallets that have open positions
3. **Position Management**: Each wallet maintains independent positions

### Benefits:
- **Diversification**: Spread risk across multiple wallets
- **Parallel Trading**: Multiple positions can be active simultaneously
- **Risk Management**: If one wallet has issues, others continue trading
- **Scalability**: Easy to add more wallets as needed

## Current Configuration
You currently have **1 wallet** configured. The system will work with just one wallet, but to get multiwallet benefits, add more wallets using the steps above.

## Testing
1. Start with 2-3 wallets for testing
2. Use small amounts initially
3. Monitor logs to see which wallet executes trades
4. Check database for per-wallet position tracking

## Monitoring
The system logs will show:
```
🏦 Initialized 3 wallets for multiwallet trading
  Wallet 1: Main_Wallet
  Wallet 2: Trading_Wallet_2  
  Wallet 3: Trading_Wallet_3
```

And during trading:
```
💰 Trading_Wallet_2 executing BUY signal
💱 Main_Wallet closing position opened at $150.25
```