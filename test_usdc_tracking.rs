// Test script to verify USDC balance change tracking is working correctly
use reqwest::Client;
use serde_json::json;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🔍 Testing USDC Balance Change Tracking");
    println!("=====================================");
    
    let client = Client::new();
    let database_url = "http://localhost:8080";
    
    // Test 1: Create a test position with USDC spent
    println!("\n1️⃣ Testing Position Creation with USDC Data");
    let create_position_payload = json!({
        "wallet_address": "TEST_WALLET_123",
        "pair": "SOL/USDC",
        "position_type": "long",
        "entry_price": 150.0,
        "quantity": 1.0,
        "usdc_spent": 152.50  // Including fees
    });
    
    let create_response = client
        .post(&format!("{}/positions", database_url))
        .json(&create_position_payload)
        .send()
        .await?;
    
    if create_response.status().is_success() {
        let response_text = create_response.text().await?;
        println!("✅ Position created successfully");
        println!("📊 Response: {}", response_text);
        
        // Extract position ID from response
        let response_json: serde_json::Value = serde_json::from_str(&response_text)?;
        if let Some(position_id) = response_json["data"]["id"].as_str() {
            println!("🆔 Position ID: {}", position_id);
            
            // Test 2: Close the position with USDC received
            println!("\n2️⃣ Testing Position Closure with USDC Data");
            let close_position_payload = json!({
                "position_id": position_id,
                "exit_price": 155.0,
                "usdc_received": 153.75  // After fees
            });
            
            let close_response = client
                .post(&format!("{}/positions/close", database_url))
                .json(&close_position_payload)
                .send()
                .await?;
            
            if close_response.status().is_success() {
                println!("✅ Position closed successfully");
                
                // Test 3: Check performance metrics for USDC-based PnL
                println!("\n3️⃣ Testing Performance Metrics (USDC-based PnL)");
                let metrics_response = client
                    .get(&format!("{}/performance/metrics", database_url))
                    .send()
                    .await?;
                
                if metrics_response.status().is_success() {
                    let metrics_text = metrics_response.text().await?;
                    let metrics_json: serde_json::Value = serde_json::from_str(&metrics_text)?;
                    
                    if let Some(total_pnl) = metrics_json["data"]["total_pnl"].as_f64() {
                        println!("✅ Performance metrics retrieved");
                        println!("💰 Total PnL: ${:.2}", total_pnl);
                        
                        // Expected PnL: $153.75 (received) - $152.50 (spent) = $1.25
                        let expected_pnl = 153.75 - 152.50;
                        println!("🎯 Expected PnL: ${:.2}", expected_pnl);
                        
                        if (total_pnl - expected_pnl).abs() < 0.01 {
                            println!("🎉 SUCCESS: USDC balance change tracking is working correctly!");
                            println!("✅ Dashboard PnL matches actual USDC flow");
                        } else {
                            println!("❌ FAILURE: PnL mismatch");
                            println!("   Expected: ${:.2}", expected_pnl);
                            println!("   Actual: ${:.2}", total_pnl);
                        }
                    } else {
                        println!("❌ Could not extract total_pnl from metrics");
                    }
                } else {
                    println!("❌ Failed to get performance metrics: {}", metrics_response.status());
                }
            } else {
                println!("❌ Failed to close position: {}", close_response.status());
            }
        } else {
            println!("❌ Could not extract position ID from response");
        }
    } else {
        println!("❌ Failed to create position: {}", create_response.status());
        let error_text = create_response.text().await?;
        println!("Error: {}", error_text);
    }
    
    println!("\n🔍 Test completed");
    Ok(())
}