use crate::error::CLMMError;
use crate::math::tick_math::{U256, Q96};
use crate::math::FixedPointMath;
use crate::state::Pool;
use solana_program::program_error::ProgramError;

/// Advanced price impact calculator with slippage protection
pub struct PriceImpactCalculator;

impl PriceImpactCalculator {
    /// Calculate price impact of a swap
    pub fn calculate_price_impact(
        pool: &Pool,
        amount_in: U256,
        zero_for_one: bool,
    ) -> Result<PriceImpactResult, ProgramError> {
        if pool.liquidity == U256_ZERO {
            return Ok(PriceImpactResult {
                impact_bps: 10000, // 100% impact
                expected_price: 0.0,
                price_change: f64::INFINITY,
                severity: ImpactSeverity::Critical,
            });
        }

        let current_price = FixedPointMath::sqrt_price_x96_to_price(pool.sqrt_price_x96);
        let amount_out = Self::estimate_swap_output(pool, amount_in, zero_for_one)?;

        if amount_out == U256_ZERO {
            return Ok(PriceImpactResult {
                impact_bps: 10000,
                expected_price: 0.0,
                price_change: f64::INFINITY,
                severity: ImpactSeverity::Critical,
            });
        }

        let expected_price = if zero_for_one {
            // Token0 -> Token1: price increases
            current_price * (amount_in as f64 / amount_out as f64)
        } else {
            // Token1 -> Token0: price decreases
            current_price * (amount_out as f64 / amount_in as f64)
        };

        let price_change = ((expected_price - current_price) / current_price) * 100.0;
        let impact_bps = (price_change.abs() * 100.0) as u32;

        let severity = Self::classify_impact_severity(impact_bps);

        Ok(PriceImpactResult {
            impact_bps,
            expected_price,
            price_change,
            severity,
        })
    }

    /// Estimate swap output without executing the swap
    pub fn estimate_swap_output(
        pool: &Pool,
        amount_in: U256,
        zero_for_one: bool,
    ) -> Result<U256, ProgramError> {
        let current_liquidity = pool.liquidity;
        let current_sqrt_price = pool.sqrt_price_x96;

        if current_liquidity == U256_ZERO {
            return Ok(U256_ZERO);
        }

        // Calculate fee
        let fee_amount = amount_in * U256::from(pool.fee) / U256::from(10000);
        let amount_after_fee = amount_in - fee_amount;

        if zero_for_one {
            // Token0 -> Token1
            let price_ratio = current_sqrt_price * current_sqrt_price / Q96;
            Ok(amount_after_fee * Q96 / price_ratio)
        } else {
            // Token1 -> Token0
            let price_ratio = Q96 * Q96 / (current_sqrt_price * current_sqrt_price);
            Ok(amount_after_fee * price_ratio / Q96)
        }
    }

    /// Calculate optimal swap amount to minimize price impact
    pub fn calculate_optimal_swap_amount(
        pool: &Pool,
        target_price_impact_bps: u32,
        zero_for_one: bool,
    ) -> Result<U256, ProgramError> {
        if pool.liquidity == U256_ZERO {
            return Ok(U256_ZERO);
        }

        // Binary search for optimal amount
        let mut low = U256::one();
        let mut high = pool.liquidity / U256::from(10); // Cap at 10% of liquidity
        let mut optimal_amount = U256_ZERO;

        for _ in 0..64 {
            let mid = (low + high) / U256::from(2);
            let impact_result = Self::calculate_price_impact(pool, mid, zero_for_one)?;

            if impact_result.impact_bps <= target_price_impact_bps {
                optimal_amount = mid;
                low = mid + U256::one();
            } else {
                high = mid - U256::one();
            }
        }

        Ok(optimal_amount)
    }

    /// Get recommended slippage protection based on price impact
    pub fn get_recommended_slippage_bps(impact_bps: u32) -> u32 {
        match Self::classify_impact_severity(impact_bps) {
            ImpactSeverity::Negligible => 10,    // 0.1%
            ImpactSeverity::Low => 50,           // 0.5%
            ImpactSeverity::Medium => 100,       // 1%
            ImpactSeverity::High => 200,         // 2%
            ImpactSeverity::Critical => 500,     // 5%
        }
    }

    /// Classify impact severity
    fn classify_impact_severity(impact_bps: u32) -> ImpactSeverity {
        match impact_bps {
            0..=50 => ImpactSeverity::Negligible,
            51..=200 => ImpactSeverity::Low,
            201..=500 => ImpactSeverity::Medium,
            501..=2000 => ImpactSeverity::High,
            _ => ImpactSeverity::Critical,
        }
    }

    /// Calculate impermanent loss for a position (bonus feature)
    pub fn calculate_impermanent_loss(
        position_lower_sqrt_price: U256,
        position_upper_sqrt_price: U256,
        current_sqrt_price: U256,
        initial_liquidity: U256,
    ) -> Result<f64, ProgramError> {
        let position_price_lower = FixedPointMath::sqrt_price_x96_to_price(position_lower_sqrt_price);
        let position_price_upper = FixedPointMath::sqrt_price_x96_to_price(position_upper_sqrt_price);
        let current_price = FixedPointMath::sqrt_price_x96_to_price(current_sqrt_price);

        // Calculate amounts at current price
        let (amount0_current, amount1_current) = FixedPointMath::get_amounts_for_liquidity(
            position_lower_sqrt_price,
            position_upper_sqrt_price,
            initial_liquidity,
        );

        // Calculate amounts if price stayed the same (HODL)
        let hodl_amount0 = FixedPointMath::get_amount0_for_liquidity(
            position_lower_sqrt_price,
            position_upper_sqrt_price,
            initial_liquidity,
        );
        let hodl_amount1 = FixedPointMath::get_amount1_for_liquidity(
            position_lower_sqrt_price,
            position_upper_sqrt_price,
            initial_liquidity,
        );

        // Calculate current value vs HODL value
        let current_value = amount0_current as f64 + amount1_current as f64 * current_price;
        let hodl_value = hodl_amount0 as f64 + hodl_amount1 as f64 * current_price;

        if hodl_value == 0.0 {
            return Ok(0.0);
        }

        let impermanent_loss = (current_value - hodl_value) / hodl_value;
        Ok(impermanent_loss)
    }
}

/// Price impact analysis result
#[derive(Debug, Clone)]
pub struct PriceImpactResult {
    pub impact_bps: u32,        // Impact in basis points
    pub expected_price: f64,    // Expected price after swap
    pub price_change: f64,      // Price change percentage
    pub severity: ImpactSeverity,
}

/// Severity levels for price impact
#[derive(Debug, Clone, PartialEq)]
pub enum ImpactSeverity {
    Negligible,  // < 0.5%
    Low,         // 0.5% - 2%
    Medium,      // 2% - 5%
    High,        // 5% - 20%
    Critical,    // > 20%
}

impl ImpactSeverity {
    pub fn color_code(&self) -> &'static str {
        match self {
            ImpactSeverity::Negligible => "🟢",
            ImpactSeverity::Low => "🟡",
            ImpactSeverity::Medium => "🟠",
            ImpactSeverity::High => "🔴",
            ImpactSeverity::Critical => "💀",
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            ImpactSeverity::Negligible => "Minimal impact - safe to proceed",
            ImpactSeverity::Low => "Low impact - proceed with caution",
            ImpactSeverity::Medium => "Medium impact - consider reducing amount",
            ImpactSeverity::High => "High impact - strongly recommend reducing amount",
            ImpactSeverity::Critical => "Critical impact - swap may fail or be unprofitable",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use solana_program::pubkey::Pubkey;

    #[test]
    fn test_price_impact_calculation() {
        let pool = create_test_pool();
        let amount_in = U256::from(1000u64);
        let result = PriceImpactCalculator::calculate_price_impact(&pool, amount_in, true).unwrap();

        assert!(result.impact_bps >= 0 && result.impact_bps <= 10000);
        assert!(result.expected_price >= 0.0);
    }

    #[test]
    fn test_optimal_swap_amount() {
        let pool = create_test_pool();
        let target_impact = 100; // 1%
        let optimal_amount = PriceImpactCalculator::calculate_optimal_swap_amount(&pool, target_impact, true).unwrap();

        // Should get some reasonable amount
        assert!(optimal_amount > U256_ZERO);
    }

    #[test]
    fn test_impact_severity_classification() {
        assert_eq!(
            PriceImpactCalculator::classify_impact_severity(25),
            ImpactSeverity::Negligible
        );
        assert_eq!(
            PriceImpactCalculator::classify_impact_severity(150),
            ImpactSeverity::Low
        );
        assert_eq!(
            PriceImpactCalculator::classify_impact_severity(350),
            ImpactSeverity::Medium
        );
        assert_eq!(
            PriceImpactCalculator::classify_impact_severity(1000),
            ImpactSeverity::High
        );
        assert_eq!(
            PriceImpactCalculator::classify_impact_severity(5000),
            ImpactSeverity::Critical
        );
    }

    fn create_test_pool() -> Pool {
        let token_a = Pubkey::new_unique();
        let token_b = Pubkey::new_unique();
        let initial_price = U256([1000000000000000000000000, 0, 0, 0]); // 1e21

        Pool::new(token_a, token_b, 300, 60, initial_price).unwrap()
    }
}
