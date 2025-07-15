# ML Trade Data Query Script

This script allows you to manually check and verify that machine learning trade data is being stored correctly in the database. It provides various query options and detailed analysis of the ML trade history.

## Prerequisites

The script requires the following tools to be installed:

- `jq` - JSON processor
- `curl` - HTTP client
- `bc` - Basic calculator (usually pre-installed)

### Installation

**Ubuntu/Debian:**
```bash
sudo apt-get install jq curl
```

**macOS:**
```bash
brew install jq curl
```

## Usage

### Basic Usage

```bash
# Query ML trades for SOLUSDC (default)
./query_trades_ml.sh

# Query ML trades for a specific pair
./query_trades_ml.sh --pair SOLUSDC

# Query with custom limit
./query_trades_ml.sh --pair SOLUSDC --limit 50
```

### Advanced Options

```bash
# Show ML trade statistics
./query_trades_ml.sh --pair SOLUSDC --stats

# Show ML system status
./query_trades_ml.sh --pair SOLUSDC --status

# Verify trade data integrity
./query_trades_ml.sh --pair SOLUSDC --verify

# Show detailed trade information
./query_trades_ml.sh --pair SOLUSDC --details

# Export trades to CSV
./query_trades_ml.sh --pair SOLUSDC --export csv

# Export trades to JSON
./query_trades_ml.sh --pair SOLUSDC --export json

# Export with custom filename
./query_trades_ml.sh --pair SOLUSDC --export csv --output my_trades.csv
```

### Custom Database URL

```bash
# Use custom database URL
./query_trades_ml.sh --pair SOLUSDC --database-url http://localhost:8080
```

### Combined Options

```bash
# Multiple options together
./query_trades_ml.sh --pair SOLUSDC --limit 30 --stats --verify --details
```

## Output Examples

### Basic Trade List
```
🤖 ML Trade History (5 trades):
================================================================================================
 1. ✅ 💰 SOLUSDC | Entry: $123.4567 | Exit: $124.5678 | PnL: +0.90% | Duration: 15.2m | Regime: Trending | Trend: 0.750 | Vol: 0.025 | Time: 2024-01-15 14:30:25
 2. ❌ 💸 SOLUSDC | Entry: $124.5678 | Exit: $123.4567 | PnL: -0.89% | Duration: 8.5m | Regime: Volatile | Trend: 0.320 | Vol: 0.045 | Time: 2024-01-15 15:45:12
```

### Statistics Output
```
📊 ML Trade Statistics:
==================================================
Total Trades: 25
Win Rate: 68.0%
Average PnL: +0.45%
Average Win: +1.23%
Average Loss: -0.78%
```

### Verification Output
```
🔍 Verifying trade data integrity...
✅ All trade data is valid!

📊 Verification Summary:
   Total trades checked: 25
   Errors: 0
   Warnings: 2
```

## Data Verification

The script performs comprehensive data validation including:

- **Required Fields**: Checks for all required fields (id, pair, entry_price, etc.)
- **Data Types**: Validates numeric values and boolean flags
- **Value Ranges**: Ensures prices > 0, durations >= 0, etc.
- **Time Consistency**: Verifies exit time > entry time
- **Market Regime**: Validates regime values (Consolidating, Trending, Volatile)
- **Duration Calculation**: Cross-checks stored vs calculated duration

## Export Formats

### CSV Export
Creates a CSV file with headers and all trade data:
```csv
id,pair,entry_price,exit_price,pnl,duration_seconds,entry_time,exit_time,success,market_regime,trend_strength,volatility,created_at
uuid-1,SOLUSDC,123.4567,124.5678,0.009,912,2024-01-15T14:30:25Z,2024-01-15T14:45:37Z,true,Trending,0.750,0.025,2024-01-15T14:45:37Z
```

### JSON Export
Creates a JSON file with the complete trade data structure.

## Error Handling

The script provides clear error messages for:

- **Network Issues**: Connection problems to database service
- **API Errors**: HTTP errors from the database service
- **Data Validation**: Invalid or missing trade data
- **Dependency Issues**: Missing required tools (jq, curl, bc)

## Configuration

Default settings can be modified in the script:

```bash
# Configuration section
DEFAULT_DATABASE_URL="http://localhost:8080"
DEFAULT_PAIR="SOLUSDC"
DEFAULT_LIMIT=20
```

## Troubleshooting

### Common Issues

1. **"jq is required but not installed"**
   ```bash
   sudo apt-get install jq  # Ubuntu/Debian
   brew install jq          # macOS
   ```

2. **"HTTP 404 error"**
   - Check if the database service is running
   - Verify the database URL is correct
   - Ensure the trading pair exists

3. **"No ML trades found"**
   - The pair may not have any ML trade history yet
   - Check if ML is enabled in the trading system
   - Verify trades are being recorded

4. **"Data validation errors"**
   - Check the database schema matches expected format
   - Verify ML trade recording is working correctly
   - Review the specific error messages for details

### Debug Mode

For debugging, you can run the script with verbose output:

```bash
# Enable bash debug mode
bash -x ./query_trades_ml.sh --pair SOLUSDC
```

## Integration with Trading System

This script is designed to work with the trading system's ML trade recording functionality. The ML strategy in `trading-logic/src/ml_strategy.rs` records trades via:

```rust
pub async fn record_trade_with_context(&mut self, trade_result: TradeResult, pair: &str, market_regime: &str, trend_strength: f64, volatility: f64)
```

The script queries the same data that this function stores, allowing you to verify that:

1. Trades are being recorded correctly
2. ML context data (market regime, trend strength, volatility) is accurate
3. Data integrity is maintained
4. Performance metrics are calculated correctly

## File Structure

```
tirade/
├── query_trades_ml.sh          # Main bash script
├── query_trades_ml.py          # Python alternative
└── README_query_trades_ml.md   # This documentation
```

## Contributing

To extend the script functionality:

1. Add new validation rules in `verify_trade_data()`
2. Add new export formats in `export_trades()`
3. Add new display options in the main function
4. Update this README with new features 