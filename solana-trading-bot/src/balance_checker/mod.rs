use crate::error::{Result, TradingBotError};
use solana_client::rpc_client::RpcClient;
use solana_client::rpc_request::TokenAccountsFilter;
use solana_sdk::{
    pubkey::Pubkey,
    signature::{Keypair, Signer},
};
use solana_sdk::program_pack::Pack;
use spl_token::state::Account as TokenAccount;
use std::str::FromStr;
use solana_account_decoder::{UiAccountData, UiAccountEncoding};
use base64::Engine;

#[derive(Debug)]
pub struct Wallet {
    pub keypair: Keypair,
    pub pubkey: Pubkey,
}

impl Wallet {
    pub fn from_private_key(private_key: &str) -> Result<Self> {
        let private_key = private_key.trim();
        let keypair_bytes = if private_key.starts_with('[') {
            serde_json::from_str::<Vec<u8>>(private_key)
                .map_err(|e| TradingBotError::PrivateKey(format!("Invalid private key array format: {}", e)))?
        } else {
            bs58::decode(private_key)
                .into_vec()
                .map_err(|e| TradingBotError::PrivateKey(format!("Invalid base58 private key: {}", e)))?
        };
        if keypair_bytes.len() != 64 {
            return Err(TradingBotError::PrivateKey("Invalid private key length".to_string()));
        }
        let keypair = Keypair::from_bytes(&keypair_bytes)
            .map_err(|e| TradingBotError::PrivateKey(format!("Failed to create keypair: {}", e)))?;
        let pubkey = keypair.pubkey();
        Ok(Self { keypair, pubkey })
    }

    pub fn get_balance(&self, client: &RpcClient) -> Result<f64> {
        let balance = client
            .get_balance(&self.pubkey)
            .map_err(TradingBotError::SolanaClient)?;
        Ok(balance as f64 / 1_000_000_000.0)
    }

    pub fn get_token_balance(&self, client: &RpcClient, token_mint: &str) -> Result<f64> {
        let mint_pubkey = Pubkey::from_str(token_mint)
            .map_err(|e| TradingBotError::Validation(format!("Invalid token mint: {}", e)))?;
        let accounts = client
            .get_token_accounts_by_owner(&self.pubkey, TokenAccountsFilter::Mint(mint_pubkey))
            .map_err(TradingBotError::SolanaClient)?;
        let mut total_balance = 0.0;
        for keyed_account in accounts {
            match &keyed_account.account.data {
                UiAccountData::Binary(data, encoding) if *encoding == UiAccountEncoding::Base64 => {
                    if let Ok(decoded) = base64::engine::general_purpose::STANDARD.decode(data) {
                        if let Ok(token_account) = TokenAccount::unpack(&decoded) {
                            total_balance += token_account.amount as f64 / 1_000_000.0;
                        }
                    }
                }
                UiAccountData::Json(data) => {
                    if data.program == "spl-token" {
                        if let Some(info) = data.parsed.get("info") {
                            if let Some(token_amount) = info.get("tokenAmount") {
                                if let Some(amount_str) = token_amount.get("amount") {
                                    if let Ok(amount) = amount_str.as_str().unwrap_or("0").parse::<f64>() {
                                        total_balance += amount / 1_000_000.0;
                                    }
                                }
                            }
                        }
                    }
                }
                _ => {}
            }
        }
        Ok(total_balance)
    }

    pub fn get_wallet_info(&self, client: &RpcClient, usdc_mint: &str) -> Result<WalletInfo> {
        let sol_balance = self.get_balance(client)?;
        let usdc_balance = self.get_token_balance(client, usdc_mint)?;
        Ok(WalletInfo {
            pubkey: self.pubkey.to_string(),
            sol_balance,
            usdc_balance,
            timestamp: chrono::Utc::now(),
        })
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct WalletInfo {
    pub pubkey: String,
    pub sol_balance: f64,
    pub usdc_balance: f64,
    pub timestamp: chrono::DateTime<chrono::Utc>,
} 