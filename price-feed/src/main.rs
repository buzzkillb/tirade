use anyhow::Result;
use dotenv::dotenv;
use reqwest::Client;
use std::env;
use std::time::Duration;
use tokio::time;

#[tokio::main]
async fn main() -> Result<()> {
    dotenv().ok();
    let pyth_interval = env::var("PYTH_INTERVAL_SECS")
        .unwrap_or_else(|_| "1".to_string())
        .parse::<u64>()
        .expect("Invalid PYTH_INTERVAL_SECS");
    let jup_interval = env::var("JUP_INTERVAL_SECS")
        .unwrap_or_else(|_| "10".to_string())
        .parse::<u64>()
        .expect("Invalid JUP_INTERVAL_SECS");

    let client = Client::new();

    let pyth_task = tokio::spawn(fetch_pyth_loop(client.clone(), pyth_interval));
    let jup_task = tokio::spawn(fetch_jup_loop(client.clone(), jup_interval));

    let _ = tokio::join!(pyth_task, jup_task);
    Ok(())
}

async fn fetch_pyth_loop(client: Client, interval: u64) {
    // Pyth Hermes API for SOL/USD price feed
    let feed_id = "0xef0d8b6fda2ceba41da15d4095d1da392a0d2f8ed0c6c7bc0f4cfac8c280b56d";
    let url = format!("https://hermes.pyth.network/api/latest_price_feeds?ids[]={}", feed_id);
    
    let mut ticker = time::interval(Duration::from_secs(interval));
    loop {
        ticker.tick().await;
        
        match client.get(&url).send().await {
            Ok(resp) => {
                if resp.status().is_success() {
                    match resp.text().await {
                        Ok(text) => {
                            match serde_json::from_str::<serde_json::Value>(&text) {
                                Ok(json) => {
                                    // The response is an array, get the first item
                                    if let Some(feeds) = json.as_array() {
                                        if let Some(first_feed) = feeds.first() {
                                            if let Some(price_obj) = first_feed.get("price") {
                                                if let Some(price_str) = price_obj.get("price") {
                                                    if let Some(price_str_val) = price_str.as_str() {
                                                        if let Some(expo) = price_obj.get("expo") {
                                                            if let Some(expo_val) = expo.as_i64() {
                                                                // Parse the price string and apply the exponent
                                                                if let Ok(price_int) = price_str_val.parse::<i64>() {
                                                                    let price_float = price_int as f64 * 10_f64.powi(expo_val as i32);
                                                                    println!("[PYTH] SOL/USD: ${:.4}", price_float);
                                                                }
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                                Err(_) => {}
                            }
                        }
                        Err(_) => {}
                    }
                }
            }
            Err(_) => {}
        }
    }
}

async fn fetch_jup_loop(client: Client, interval: u64) {
    // Jupiter v6 Quote API for SOL/USDC price
    let sol_mint = "So11111111111111111111111111111111111111112";
    let usdc_mint = "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v";
    // Using 1 SOL (1000000000 lamports) as input amount to get price
    let amount = "1000000000";
    let url = format!("https://quote-api.jup.ag/v6/quote?inputMint={}&outputMint={}&amount={}&slippageBps=50", 
                     sol_mint, usdc_mint, amount);
    
    let mut ticker = time::interval(Duration::from_secs(interval));
    loop {
        ticker.tick().await;
        match client.get(&url).send().await {
            Ok(resp) => {
                if resp.status().is_success() {
                    match resp.text().await {
                        Ok(text) => {
                            match serde_json::from_str::<serde_json::Value>(&text) {
                                Ok(json) => {
                                    if let Some(out_amount) = json.get("outAmount") {
                                        if let Some(out_amount_str) = out_amount.as_str() {
                                            if let Ok(out_amount_val) = out_amount_str.parse::<f64>() {
                                                // Convert from USDC (6 decimals) to actual USDC amount
                                                let usdc_amount = out_amount_val / 1_000_000.0;
                                                println!("[JUP] SOL/USDC: ${:.4}", usdc_amount);
                                            }
                                        }
                                    }
                                }
                                Err(_) => {}
                            }
                        }
                        Err(_) => {}
                    }
                }
            }
            Err(_) => {}
        }
    }
}
