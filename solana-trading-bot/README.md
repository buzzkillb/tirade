# Solana Trading Bot

A Rust-based trading bot for Solana with wallet balance checking functionality.

## Features

- Check SOL and USDC balances on Solana mainnet
- Load private key and RPC URL from environment variables
- Support for both base58 and array format private keys
- Async/await support for better performance

## Prerequisites

- Rust (latest stable version)
- Solana CLI tools installed
- A Solana wallet with private key

## Setup

1. **Clone and navigate to the project:**
   ```bash
   cd solana-trading-bot
   ```

2. **Install dependencies:**
   ```bash
   cargo build
   ```

3. **Set up environment variables:**
   ```bash
   cp .env.example .env
   ```

4. **Edit the .env file with your credentials:**
   ```bash
   nano .env
   ```

## Getting Your Private Key

### Method 1: Using Solana CLI (Recommended)
```bash
# Generate a new keypair
solana-keygen new --outfile keypair.json

# View the private key (base58 format)
cat keypair.json

# Copy the private key array to your .env file
```

### Method 2: From Existing Wallet
If you already have a wallet, you can export the private key:
```bash
# Export private key from existing wallet
solana-keygen pubkey --outfile keypair.json
```

## Environment Variables

- `SOLANA_PRIVATE_KEY`: Your wallet's private key (base58 encoded or array format)
- `SOLANA_RPC_URL`: Solana RPC endpoint (optional, defaults to mainnet-beta)

## Usage

### Check Wallet Balances
```bash
cargo run -- --balance
```

### Example Output
```
=== Wallet Balance Check ===
Wallet Address: 7xKXtg2CW87d97TXJSDpbD5jBkheTqA83TZRuJosgAsU
SOL Balance: 1.234567 SOL
USDC Balance: 100.000000 USDC
===========================
```

## RPC Endpoints

The bot supports various RPC endpoints:

- **Public RPCs:**
  - `https://api.mainnet-beta.solana.com` (default)
  - `https://solana-api.projectserum.com`
  - `https://rpc.ankr.com/solana`

- **Private RPCs:**
  - Use your own RPC endpoint for better performance and rate limits

## Security Notes

- Never commit your `.env` file to version control
- Keep your private key secure and never share it
- Consider using a dedicated wallet for trading bot operations
- Use environment variables for sensitive data in production

## Building for Production

```bash
# Release build
cargo build --release

# Run the release binary
./target/release/solana-trading-bot --balance
```

## Error Handling

The bot includes comprehensive error handling for:
- Invalid private key formats
- Network connection issues
- RPC endpoint failures
- Missing environment variables

## Next Steps

This is the foundation for your trading bot. Future features could include:
- Price monitoring
- Automated trading strategies
- Order placement
- Portfolio tracking
- Risk management

## License

MIT License - feel free to modify and distribute as needed. 