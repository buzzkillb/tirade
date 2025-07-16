use crate::models::{PositionDb};
use crate::database_service::DatabaseService;
use anyhow::{Result, anyhow};
use chrono::{DateTime, Utc};
use tracing::{info, warn, error};

#[derive(Debug, Clone)]
pub struct Position {
    pub position_id: Option<String>,
    pub entry_price: f64,
    pub entry_time: DateTime<Utc>,
    pub quantity: f64,
    pub position_type: PositionType,
}

#[derive(Debug, Clone)]
pub enum PositionType {
    Long,
    Short,
}

pub struct PositionManager {
    positions: Vec<Option<Position>>,
}

impl PositionManager {
    pub fn new(wallet_count: usize) -> Self {
        Self {
            positions: vec![None; wallet_count],
        }
    }

    pub fn get_position(&self, wallet_index: usize) -> Option<&Position> {
        self.positions.get(wallet_index).and_then(|p| p.as_ref())
    }

    pub fn set_position(&mut self, wallet_index: usize, position: Option<Position>) {
        if wallet_index < self.positions.len() {
            self.positions[wallet_index] = position;
        }
    }

    pub fn has_position(&self, wallet_index: usize) -> bool {
        self.positions.get(wallet_index).map_or(false, |p| p.is_some())
    }

    pub fn get_active_position_count(&self) -> usize {
        self.positions.iter().filter(|p| p.is_some()).count()
    }

    pub fn clear_position(&mut self, wallet_index: usize) {
        if wallet_index < self.positions.len() {
            self.positions[wallet_index] = None;
        }
    }

    pub async fn recover_positions(&mut self, database: &DatabaseService, wallet_addresses: &[String]) -> Result<()> {
        info!("🔄 Recovering positions from database for {} wallets...", wallet_addresses.len());
        
        for (wallet_index, wallet_address) in wallet_addresses.iter().enumerate() {
            match database.fetch_open_positions_for_wallet(wallet_address).await {
                Ok(Some(position_db)) => {
                    let position = Position {
                        position_id: Some(position_db.id.clone()),
                        entry_price: position_db.entry_price,
                        entry_time: position_db.entry_time,
                        quantity: position_db.quantity,
                        position_type: match position_db.position_type.as_str() {
                            "long" => PositionType::Long,
                            "short" => PositionType::Short,
                            _ => {
                                warn!("Invalid position type: {}", position_db.position_type);
                                continue;
                            }
                        },
                    };
                    
                    self.positions[wallet_index] = Some(position.clone());
                    info!("📈 Wallet {} recovered position: Entry ${:.4}", 
                          wallet_index + 1, position.entry_price);
                }
                Ok(None) => {
                    info!("💤 Wallet {} no open positions", wallet_index + 1);
                }
                Err(e) => {
                    warn!("❌ Wallet {} failed to recover positions: {}", wallet_index + 1, e);
                }
            }
        }
        
        Ok(())
    }

    pub fn get_wallet_stats(&self, wallet_names: &[String]) -> Vec<WalletStats> {
        self.positions.iter().enumerate().map(|(i, position)| {
            let wallet_name = wallet_names.get(i)
                .map(|s| s.clone())
                .unwrap_or_else(|| format!("Wallet_{}", i + 1));
                
            WalletStats {
                wallet_index: i,
                wallet_name,
                has_position: position.is_some(),
                position_entry_price: position.as_ref().map(|p| p.entry_price),
                position_age_hours: position.as_ref().map(|p| {
                    (Utc::now() - p.entry_time).num_hours()
                }),
            }
        }).collect()
    }
}

#[derive(Debug)]
pub struct WalletStats {
    pub wallet_index: usize,
    pub wallet_name: String,
    pub has_position: bool,
    pub position_entry_price: Option<f64>,
    pub position_age_hours: Option<i64>,
}