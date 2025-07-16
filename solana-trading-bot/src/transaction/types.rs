use clap::Parser;
use serde_json::Value;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct Args {
    /// Amount in USDC to swap (default: 0.10)
    #[arg(short, long, default_value = "0.10")]
    pub amount_usdc: f64,
    
    /// Direction: usdc-to-sol or sol-to-usdc
    #[arg(long, default_value = "usdc-to-sol")]
    pub direction: String,
    
    /// Dry run - don't actually execute the transaction
    #[arg(short, long)]
    pub dry_run: bool,
    
    /// Slippage tolerance in basis points (default: 50 = 0.5%)
    #[arg(short, long, default_value = "50")]
    pub slippage_bps: u32,
}

#[derive(Debug)]
pub struct JupiterQuote {
    pub input_amount: String,
    pub output_amount: String,
    pub execution_price: f64,  // Actual execution price from Jupiter
    pub price_impact: f64,
    #[allow(dead_code)]
    pub routes: Vec<Value>,
    pub quote_data: Value, // Store the full quote response
} 