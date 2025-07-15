# ML Trade Query Script Summary

## What Was Created

I've created a comprehensive bash script (`query_trades_ml.sh`) that allows you to manually check and verify that machine learning trade data is being stored correctly in your database.

## Key Features

### 🔍 **Query Capabilities**
- Fetch ML trade history for any trading pair
- Configurable limit for number of trades to retrieve
- Support for custom database URLs

### 📊 **Analysis Features**
- **Statistics**: Win rate, average PnL, average wins/losses
- **ML Status**: System status, confidence thresholds, position sizes
- **Data Verification**: Comprehensive validation of trade data integrity
- **Detailed View**: Show full trade details including IDs and timestamps

### 📁 **Export Options**
- **CSV Export**: For spreadsheet analysis
- **JSON Export**: For programmatic processing
- Custom filename support

### ✅ **Data Validation**
The script performs thorough validation including:
- Required field presence
- Data type validation
- Value range checks (prices > 0, durations >= 0)
- Time consistency (exit > entry time)
- Market regime validation
- Duration calculation cross-checking

## Usage Examples

```bash
# Basic query
./query_trades_ml.sh --pair SOLUSDC

# With statistics and verification
./query_trades_ml.sh --pair SOLUSDC --stats --verify

# Export to CSV for analysis
./query_trades_ml.sh --pair SOLUSDC --export csv

# Show detailed information
./query_trades_ml.sh --pair SOLUSDC --details --limit 50
```

## Integration with Your System

The script queries the same ML trade data that your `MLStrategy` in `trading-logic/src/ml_strategy.rs` records via:

```rust
pub async fn record_trade_with_context(&mut self, trade_result: TradeResult, pair: &str, market_regime: &str, trend_strength: f64, volatility: f64)
```

## What You Can Verify

1. **Trade Recording**: Are trades being saved to the database correctly?
2. **ML Context**: Is market regime, trend strength, and volatility data accurate?
3. **Data Integrity**: Are all required fields present and valid?
4. **Performance**: Are win rates and PnL calculations correct?
5. **Timing**: Are entry/exit times and durations consistent?

## Prerequisites

The script requires:
- `jq` (JSON processor)
- `curl` (HTTP client)
- `bc` (calculator - usually pre-installed)

Install with: `sudo apt-get install jq curl`

## Files Created

1. **`query_trades_ml.sh`** - Main bash script
2. **`query_trades_ml.py`** - Python alternative (if you prefer Python)
3. **`README_query_trades_ml.md`** - Comprehensive documentation
4. **`ML_TRADE_QUERY_SUMMARY.md`** - This summary

## Next Steps

1. **Test the script** when you have ML trades in your database
2. **Run verification** to ensure data integrity
3. **Export data** for external analysis if needed
4. **Monitor regularly** to catch any data issues early

The script is ready to use and will help you ensure that your ML trade data is being stored correctly and that the machine learning system is working as expected! 