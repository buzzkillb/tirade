// Test script to verify multiwallet configuration
use std::env;

fn main() {
    // Load .env file
    dotenv::from_path("../.env").ok();
    
    println!("🔍 Testing Multiwallet Configuration...\n");
    
    // Test 1: Check for multiwallet keys
    if let Ok(keys_json) = env::var("SOLANA_PRIVATE_KEYS") {
        match serde_json::from_str::<Vec<String>>(&keys_json) {
            Ok(keys) => {
                println!("✅ Found {} wallet private keys", keys.len());
                for (i, _) in keys.iter().enumerate() {
                    println!("   - Wallet {}: [PRIVATE_KEY_CONFIGURED]", i + 1);
                }
            }
            Err(e) => {
                println!("❌ Error parsing SOLANA_PRIVATE_KEYS: {}", e);
                println!("   Format should be: [\"key1\", \"key2\", \"key3\"]");
            }
        }
    } else {
        println!("⚠️  SOLANA_PRIVATE_KEYS not found, checking fallback...");
        
        if let Ok(_single_key) = env::var("SOLANA_PRIVATE_KEY") {
            println!("✅ Found single wallet fallback configuration");
            println!("   - Will operate with 1 wallet");
        } else {
            println!("❌ No wallet configuration found!");
        }
    }
    
    // Test 2: Check for wallet names
    if let Ok(names_json) = env::var("WALLET_NAMES") {
        match serde_json::from_str::<Vec<String>>(&names_json) {
            Ok(names) => {
                println!("\n✅ Found {} wallet names:", names.len());
                for (i, name) in names.iter().enumerate() {
                    println!("   - Wallet {}: {}", i + 1, name);
                }
            }
            Err(e) => {
                println!("\n❌ Error parsing WALLET_NAMES: {}", e);
            }
        }
    } else {
        println!("\n⚠️  WALLET_NAMES not configured (will auto-generate)");
    }
    
    // Test 3: Check trading configuration
    println!("\n📊 Trading Configuration:");
    println!("   - Trading Execution: {}", env::var("ENABLE_TRADING_EXECUTION").unwrap_or("false".to_string()));
    println!("   - Position Size: {}%", env::var("POSITION_SIZE_PERCENTAGE").unwrap_or("90".to_string()).parse::<f64>().unwrap_or(0.9) * 100.0);
    println!("   - Min Confidence: {}%", env::var("MIN_CONFIDENCE_THRESHOLD").unwrap_or("35".to_string()).parse::<f64>().unwrap_or(0.35) * 100.0);
    
    println!("\n🎯 Multiwallet Status:");
    let keys_count = if let Ok(keys_json) = env::var("SOLANA_PRIVATE_KEYS") {
        serde_json::from_str::<Vec<String>>(&keys_json).map(|keys| keys.len()).unwrap_or(0)
    } else if env::var("SOLANA_PRIVATE_KEY").is_ok() {
        1
    } else {
        0
    };
    
    match keys_count {
        0 => println!("❌ No wallets configured - trading will not work"),
        1 => println!("✅ Single wallet configured - basic trading will work"),
        n => println!("✅ {} wallets configured - multiwallet trading ready!", n),
    }
    
    println!("\n💡 To add more wallets:");
    println!("   1. Generate new keypairs: solana-keygen new --outfile wallet2.json");
    println!("   2. Update SOLANA_PRIVATE_KEYS in .env with multiple keys");
    println!("   3. Optionally set WALLET_NAMES for custom names");
}