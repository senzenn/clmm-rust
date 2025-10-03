use crate::error::CLMMError;
use crate::math::tick_math::{U256, U256_ZERO};
use crate::state::Pool;
use solana_program::program_error::ProgramError;
use std::collections::VecDeque;

/// Market data point for fee calculation
#[derive(Debug, Clone)]
pub struct MarketDataPoint {
    pub timestamp: u32,
    pub price: U256,
    pub volume: U256,
    pub price_impact: u32,
}

/// Fee adjustment result
#[derive(Debug, Clone)]
pub struct FeeAdjustment {
    pub old_fee: u32,
    pub new_fee: u32,
    pub adjustment_reason: String,
    pub timestamp: u32,
}

/// Dynamic fee adjustment system that adapts fees based on market conditions
pub struct DynamicFeeEngine;

impl DynamicFeeEngine {
    /// Fee adjustment parameters for a pool
    pub const BASE_FEE_BPS: u32 = 30; // 0.30% base fee
    pub const MIN_FEE_BPS: u32 = 1;   // 0.01% minimum fee
    pub const MAX_FEE_BPS: u32 = 100; // 1.00% maximum fee
    pub const VOLATILITY_WINDOW: usize = 24; // 24 data points for volatility calculation
    pub const VOLUME_WINDOW: usize = 24;     // 24 data points for volume analysis
    pub const PRICE_IMPACT_WINDOW: usize = 12; // 12 data points for price impact analysis

    /// Calculate volatility from price history
    pub fn calculate_volatility(price_history: &VecDeque<MarketDataPoint>) -> Result<f64, ProgramError> {
        if price_history.len() < 2 {
            return Ok(0.0); // No volatility with insufficient data
        }

        let mut prices: Vec<f64> = Vec::new();
        for point in price_history {
            let price_f64 = Self::u256_to_f64(point.price)?;
            prices.push(price_f64);
        }

        let mean = prices.iter().sum::<f64>() / prices.len() as f64;
        let variance = prices.iter()
            .map(|price| (price - mean).powi(2))
            .sum::<f64>() / prices.len() as f64;

        Ok(variance.sqrt() / mean) // Coefficient of variation
    }

    /// Calculate average volume over time window
    pub fn calculate_average_volume(volume_history: &VecDeque<MarketDataPoint>) -> U256 {
        if volume_history.is_empty() {
            return U256_ZERO;
        }

        let mut sum = U256_ZERO;
        for point in volume_history {
            sum = sum + point.volume;
        }
        sum / U256::from(volume_history.len())
    }

    /// Calculate average price impact
    pub fn calculate_average_price_impact(impact_history: &VecDeque<MarketDataPoint>) -> u32 {
        if impact_history.is_empty() {
            return 0;
        }

        let sum: u32 = impact_history.iter().map(|point| point.price_impact).sum();
        sum / impact_history.len() as u32
    }

    /// Determine fee adjustment based on market conditions
    pub fn calculate_fee_adjustment(
        pool: &Pool,
        price_history: &VecDeque<MarketDataPoint>,
        volume_history: &VecDeque<MarketDataPoint>,
        impact_history: &VecDeque<MarketDataPoint>,
    ) -> Result<u32, ProgramError> {
        let current_fee = pool.fee;
        let mut adjustment_factor = 0i32;

        // Volatility-based adjustment (higher volatility = higher fees)
        let volatility = Self::calculate_volatility(price_history)?;
        if volatility > 0.05 { // 5% volatility threshold
            adjustment_factor += 20; // Increase by 0.20%
        } else if volatility < 0.01 { // 1% volatility threshold
            adjustment_factor -= 10; // Decrease by 0.10%
        }

        // Volume-based adjustment (higher volume = lower fees due to economies of scale)
        let avg_volume = Self::calculate_average_volume(volume_history);
        if avg_volume > U256::from(1_000_000_000_000u64) { // > 1M tokens
            adjustment_factor -= 15; // Decrease by 0.15%
        } else if avg_volume < U256::from(10_000_000_000u64) { // < 10K tokens
            adjustment_factor += 10; // Increase by 0.10%
        }

        // Price impact-based adjustment (high impact = higher fees)
        let avg_impact = Self::calculate_average_price_impact(impact_history);
        if avg_impact > 500 { // > 5% price impact
            adjustment_factor += 25; // Increase by 0.25%
        } else if avg_impact < 100 { // < 1% price impact
            adjustment_factor -= 10; // Decrease by 0.10%
        }

        // Calculate new fee with bounds checking
        let new_fee_i32 = current_fee as i32 + adjustment_factor;
        let new_fee = new_fee_i32.max(Self::MIN_FEE_BPS as i32).min(Self::MAX_FEE_BPS as i32) as u32;

        Ok(new_fee)
    }

    /// Update pool fee based on market conditions
    pub fn update_pool_fee(
        pool: &mut Pool,
        price_history: &VecDeque<MarketDataPoint>,
        volume_history: &VecDeque<MarketDataPoint>,
        impact_history: &VecDeque<MarketDataPoint>,
    ) -> Result<FeeAdjustment, ProgramError> {
        let old_fee = pool.fee;
        let new_fee = Self::calculate_fee_adjustment(pool, price_history, volume_history, impact_history)?;

        let reason = Self::generate_adjustment_reason(price_history, volume_history, impact_history);

        pool.fee = new_fee;

        Ok(FeeAdjustment {
            old_fee,
            new_fee,
            adjustment_reason: reason,
            timestamp: chrono::Utc::now().timestamp() as u32,
        })
    }

    /// Generate human-readable reason for fee adjustment
    pub fn generate_adjustment_reason(
        price_history: &VecDeque<MarketDataPoint>,
        volume_history: &VecDeque<MarketDataPoint>,
        impact_history: &VecDeque<MarketDataPoint>,
    ) -> String {
        let mut reasons = Vec::new();

        if let Ok(volatility) = Self::calculate_volatility(price_history) {
            if volatility > 0.05 {
                reasons.push("High market volatility".to_string());
            } else if volatility < 0.01 {
                reasons.push("Low market volatility".to_string());
            }
        }

        let avg_volume = Self::calculate_average_volume(volume_history);
        if avg_volume > U256::from(1_000_000_000_000u64) {
            reasons.push("High trading volume".to_string());
        } else if avg_volume < U256::from(10_000_000_000u64) {
            reasons.push("Low trading volume".to_string());
        }

        let avg_impact = Self::calculate_average_price_impact(impact_history);
        if avg_impact > 500 {
            reasons.push("High price impact".to_string());
        } else if avg_impact < 100 {
            reasons.push("Low price impact".to_string());
        }

        if reasons.is_empty() {
            "Market conditions stable".to_string()
        } else {
            format!("Adjustment based on: {}", reasons.join(", "))
        }
    }

    /// Convert U256 to f64 for calculations
    fn u256_to_f64(value: U256) -> Result<f64, ProgramError> {
        // Convert U256 to f64, handling overflow
        let bytes = value.0;
        let mut result = 0f64;

        for (i, &byte) in bytes.iter().enumerate() {
            if i >= 8 { // f64 can only handle up to 8 bytes precisely
                break;
            }
            result += (byte as f64) * 256f64.powi(i as i32);
        }

        if result.is_infinite() {
            return Err(CLMMError::InvalidPrice.into());
        }

        Ok(result)
    }

    /// Add new market data point and maintain rolling windows
    pub fn add_market_data(
        price_history: &mut VecDeque<MarketDataPoint>,
        volume_history: &mut VecDeque<MarketDataPoint>,
        impact_history: &mut VecDeque<MarketDataPoint>,
        new_point: MarketDataPoint,
    ) {
        // Add to price history
        price_history.push_back(new_point.clone());
        if price_history.len() > Self::VOLATILITY_WINDOW {
            price_history.pop_front();
        }

        // Add to volume history
        volume_history.push_back(new_point.clone());
        if volume_history.len() > Self::VOLUME_WINDOW {
            volume_history.pop_front();
        }

        // Add to impact history
        impact_history.push_back(new_point);
        if impact_history.len() > Self::PRICE_IMPACT_WINDOW {
            impact_history.pop_front();
        }
    }

    /// Check if fee adjustment should be triggered (every hour in production)
    pub fn should_adjust_fee(last_adjustment: u32, current_time: u32) -> bool {
        // In production, this might be every hour (3600 seconds)
        // For testing, we'll use a shorter interval
        const ADJUSTMENT_INTERVAL: u32 = 3600; // 1 hour in seconds
        current_time - last_adjustment >= ADJUSTMENT_INTERVAL
    }
}
