use crate::models::PriceFeed;
use chrono::{DateTime, Utc};

#[derive(Debug, Clone, serde::Serialize)]
pub struct TechnicalIndicators {
    pub pair: String,
    pub timestamp: DateTime<Utc>,
    pub sma_20: Option<f64>,
    pub sma_50: Option<f64>,
    pub sma_200: Option<f64>,
    pub rsi_14: Option<f64>,
    pub price_change_24h: Option<f64>,
    pub price_change_percent_24h: Option<f64>,
    pub volatility_24h: Option<f64>,
    pub current_price: f64,
}

pub fn calculate_sma(prices: &[f64], period: usize) -> Option<f64> {
    if prices.len() < period {
        return None;
    }
    
    let sum: f64 = prices.iter().rev().take(period).sum();
    Some(sum / period as f64)
}

pub fn calculate_rsi(prices: &[f64], period: usize) -> Option<f64> {
    if prices.len() < period + 1 {
        return None;
    }
    
    let mut gains = Vec::new();
    let mut losses = Vec::new();
    
    // Calculate price changes
    for i in 1..prices.len() {
        let change = prices[i] - prices[i - 1];
        if change > 0.0 {
            gains.push(change);
            losses.push(0.0);
        } else {
            gains.push(0.0);
            losses.push(-change);
        }
    }
    
    if gains.len() < period {
        return None;
    }
    
    // Calculate average gains and losses
    let avg_gain: f64 = gains.iter().rev().take(period).sum::<f64>() / period as f64;
    let avg_loss: f64 = losses.iter().rev().take(period).sum::<f64>() / period as f64;
    
    if avg_loss == 0.0 {
        return Some(100.0);
    }
    
    let rs = avg_gain / avg_loss;
    let rsi = 100.0 - (100.0 / (1.0 + rs));
    
    Some(rsi)
}

pub fn calculate_price_change_24h(prices: &[PriceFeed]) -> Option<(f64, f64)> {
    if prices.len() < 2 {
        return None;
    }
    
    let current_price = prices.last()?.price;
    let price_24h_ago = prices.first()?.price;
    
    let change = current_price - price_24h_ago;
    let change_percent = (change / price_24h_ago) * 100.0;
    
    Some((change, change_percent))
}

pub fn calculate_volatility(prices: &[f64], period: usize) -> Option<f64> {
    if prices.len() < period {
        return None;
    }
    
    let recent_prices: Vec<f64> = prices.iter().rev().take(period).cloned().collect();
    let mean = recent_prices.iter().sum::<f64>() / recent_prices.len() as f64;
    
    let variance = recent_prices.iter()
        .map(|&x| (x - mean).powi(2))
        .sum::<f64>() / recent_prices.len() as f64;
    
    Some(variance.sqrt())
}

pub fn calculate_indicators(prices: &[PriceFeed]) -> TechnicalIndicators {
    let price_values: Vec<f64> = prices.iter().map(|p| p.price).collect();
    let current_price = price_values.last().unwrap_or(&0.0);
    
    let sma_20 = calculate_sma(&price_values, 20);
    let sma_50 = calculate_sma(&price_values, 50);
    let sma_200 = calculate_sma(&price_values, 200);
    let rsi_14 = calculate_rsi(&price_values, 14);
    let volatility_24h = calculate_volatility(&price_values, 24);
    
    let (price_change_24h, price_change_percent_24h) = 
        calculate_price_change_24h(prices).unwrap_or((0.0, 0.0));
    
    TechnicalIndicators {
        pair: prices.first().map(|p| p.pair.clone()).unwrap_or_default(),
        timestamp: Utc::now(),
        sma_20,
        sma_50,
        sma_200,
        rsi_14,
        price_change_24h: Some(price_change_24h),
        price_change_percent_24h: Some(price_change_percent_24h),
        volatility_24h,
        current_price: *current_price,
    }
} 