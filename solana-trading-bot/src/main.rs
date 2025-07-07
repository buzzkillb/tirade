use anyhow::{anyhow, Result};
use clap::Parser;
use dotenv::dotenv;
use solana_client::rpc_client::RpcClient;
use solana_client::rpc_request::TokenAccountsFilter;
use solana_sdk::{
    commitment_config::CommitmentConfig,
    pubkey::Pubkey,
    signature::{Keypair, Signer},
    program_pack::Pack,
};
use spl_token::state::Account as TokenAccount;
use std::env;
use std::str::FromStr;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Check wallet balance
    #[arg(short, long)]
    balance: bool,
}

#[derive(Debug)]
struct WalletBalances {
    sol_balance: f64,
    usdc_balance: f64,
    wallet_address: String,
}

struct WalletChecker {
    client: RpcClient,
    keypair: Keypair,
}

impl WalletChecker {
    fn new() -> Result<Self> {
        dotenv().ok();
        
        let rpc_url = env::var("SOLANA_RPC_URL")
            .unwrap_or_else(|_| "https://api.mainnet-beta.solana.com".to_string());
        
        let private_key = env::var("SOLANA_PRIVATE_KEY")
            .map_err(|_| anyhow!("SOLANA_PRIVATE_KEY not found in .env file"))?;
        
        // Parse private key (assuming it's a base58 encoded string)
        let keypair = if private_key.starts_with('[') {
            // Handle array format [1,2,3,...]
            let bytes: Vec<u8> = serde_json::from_str(&private_key)
                .map_err(|_| anyhow!("Invalid private key format"))?;
            Keypair::from_bytes(&bytes)
                .map_err(|_| anyhow!("Invalid private key bytes"))?
        } else {
            // Handle base58 format
            let bytes = bs58::decode(&private_key)
                .into_vec()
                .map_err(|_| anyhow!("Invalid base58 private key"))?;
            Keypair::from_bytes(&bytes)
                .map_err(|_| anyhow!("Invalid private key bytes"))?
        };
        
        let client = RpcClient::new_with_commitment(
            rpc_url,
            CommitmentConfig::confirmed(),
        );
        
        Ok(Self { client, keypair })
    }
    
    async fn check_balances(&self) -> Result<WalletBalances> {
        let wallet_pubkey = self.keypair.pubkey();
        let wallet_address = wallet_pubkey.to_string();
        
        // Get SOL balance
        let sol_balance_lamports = self.client.get_balance(&wallet_pubkey)?;
        let sol_balance = sol_balance_lamports as f64 / 1_000_000_000.0; // Convert lamports to SOL
        
        // USDC token mint address (mainnet)
        let usdc_mint = Pubkey::from_str("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v")?;
        
        // Find USDC token account
        let usdc_balance = self.get_token_balance(&wallet_pubkey, &usdc_mint).await?;
        
        Ok(WalletBalances {
            sol_balance,
            usdc_balance,
            wallet_address,
        })
    }
    
    async fn get_token_balance(&self, wallet: &Pubkey, mint: &Pubkey) -> Result<f64> {
        // Find the token account for this mint
        let token_accounts = self.client.get_token_accounts_by_owner(
            wallet,
            TokenAccountsFilter::Mint(*mint),
        )?;

        if token_accounts.is_empty() {
            return Ok(0.0);
        }

        // The returned value is a vector of RpcKeyedAccount { pubkey, account }
        let token_account_pubkey = Pubkey::from_str(&token_accounts[0].pubkey)?;
        let token_account = self.client.get_account(&token_account_pubkey)?;

        // Deserialize the token account
        let token_account_data = TokenAccount::unpack(&token_account.data)?;

        // USDC has 6 decimal places
        let balance = token_account_data.amount as f64 / 1_000_000.0;

        Ok(balance)
    }
    
    fn print_balances(&self, balances: &WalletBalances) {
        println!("=== Wallet Balance Check ===");
        println!("Wallet Address: {}", balances.wallet_address);
        println!("SOL Balance: {:.6} SOL", balances.sol_balance);
        println!("USDC Balance: {:.6} USDC", balances.usdc_balance);
        println!("===========================");
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    
    if args.balance {
        let checker = WalletChecker::new()?;
        let balances = checker.check_balances().await?;
        checker.print_balances(&balances);
    } else {
        println!("Use --balance flag to check wallet balances");
        println!("Make sure you have a .env file with SOLANA_PRIVATE_KEY and optionally SOLANA_RPC_URL");
    }
    
    Ok(())
} 