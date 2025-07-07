use solana_client::rpc_client::RpcClient;
use solana_sdk::{
    commitment_config::CommitmentConfig,
    transaction::Transaction,
    signature::{Keypair, Signer},
    system_instruction,
};
use tracing::info;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt::init();
    
    // Load environment variables
    dotenv::dotenv().ok();
    
    let rpc_endpoints = vec![
        "https://api.mainnet-beta.solana.com",
        "https://solana-mainnet.rpc.extrnode.com",
        "https://rpc.ankr.com/solana",
        "https://jup.rpcpool.com",
    ];
    
    for endpoint in rpc_endpoints {
        info!("Testing RPC endpoint: {}", endpoint);
        
        let client = RpcClient::new_with_commitment(
            endpoint.to_string(),
            CommitmentConfig::confirmed()
        );
        
        // Test basic connectivity
        match client.get_version() {
            Ok(version) => {
                info!("✅ Connected successfully. Version: {:?}", version);
                
                // Test with a large transaction to see size limits
                let keypair = Keypair::new();
                let recent_blockhash = match client.get_latest_blockhash() {
                    Ok(bh) => {
                        info!("✅ Got recent blockhash: {}", bh);
                        bh
                    }
                    Err(e) => {
                        info!("❌ Failed to get blockhash: {}", e);
                        continue;
                    }
                };
                
                // Create a transaction with many instructions to test size limits
                let mut instructions = Vec::new();
                for i in 0..50 { // Try with 50 instructions
                    instructions.push(
                        system_instruction::transfer(
                            &keypair.pubkey(),
                            &keypair.pubkey(),
                            1
                        )
                    );
                }
                
                let transaction = Transaction::new_signed_with_payer(
                    &instructions,
                    Some(&keypair.pubkey()),
                    &[&keypair],
                    recent_blockhash
                );
                
                let serialized_size = transaction.message.serialize().len();
                info!("📏 Transaction size: {} bytes", serialized_size);
                
                // Try to send the transaction (it will fail, but we can see the error)
                match client.send_transaction(&transaction) {
                    Ok(_) => {
                        info!("✅ Large transaction accepted! Size: {} bytes", serialized_size);
                    }
                    Err(e) => {
                        let error_str = e.to_string();
                        if error_str.contains("too large") {
                            info!("❌ Transaction too large: {} bytes", serialized_size);
                            // Extract size limit from error if possible
                            if let Some(size_info) = extract_size_limit(&error_str) {
                                info!("📋 Size limit info: {}", size_info);
                            }
                        } else {
                            info!("❌ Other error: {}", e);
                        }
                    }
                }
                
            }
            Err(e) => {
                info!("❌ Failed to connect: {}", e);
            }
        }
        
        info!("---");
    }
    
    Ok(())
}

fn extract_size_limit(error_msg: &str) -> Option<String> {
    // Look for size limit information in the error message
    if error_msg.contains("too large") {
        // Extract the size information if present
        if let Some(start) = error_msg.find("(") {
            if let Some(end) = error_msg.find(")") {
                return Some(error_msg[start..=end].to_string());
            }
        }
    }
    None
} 