use crate::error::CLMMError;
use crate::math::tick_math::{U256, Q96, U256_ZERO};
use crate::math::fixed_point::FixedPointMath;
use crate::math::dynamic_fee::{DynamicFeeEngine, MarketDataPoint};
use crate::math::mev_protection::{MevProtectionEngine, OracleObservation};
use crate::state::Pool;
use solana_program::program_error::ProgramError;
use std::collections::VecDeque;

/// Advanced swap engine with price impact calculation and slippage protection
pub struct SwapEngine;

impl SwapEngine {
    /// Update dynamic fees based on market conditions
    pub fn update_dynamic_fees(
        pool: &mut Pool,
        price_history: &mut VecDeque<MarketDataPoint>,
        volume_history: &mut VecDeque<MarketDataPoint>,
        impact_history: &mut VecDeque<MarketDataPoint>,
        current_timestamp: u32,
        current_price: U256,
        swap_volume: U256,
        price_impact: u32,
    ) -> Result<bool, ProgramError> {
        // Check if dynamic fee adjustment is enabled and if enough time has passed
        if !pool.dynamic_fee_enabled {
            return Ok(false);
        }

        if !DynamicFeeEngine::should_adjust_fee(pool.last_fee_adjustment, current_timestamp) {
            return Ok(false);
        }

        // Add current market data to history
        let market_data = MarketDataPoint {
            timestamp: current_timestamp,
            price: current_price,
            volume: swap_volume,
            price_impact,
        };

        DynamicFeeEngine::add_market_data(
            price_history,
            volume_history,
            impact_history,
            market_data,
        );

        // Calculate and apply fee adjustment
        let adjustment = DynamicFeeEngine::update_pool_fee(
            pool,
            price_history,
            volume_history,
            impact_history,
        )?;

        // Update the last adjustment timestamp
        pool.last_fee_adjustment = current_timestamp;

        Ok(true) // Fee was adjusted
    }
    /// Execute a swap with advanced features, dynamic fee adjustment, and MEV protection
    pub fn execute_swap(
        pool: &mut Pool,
        amount_in: U256,
        zero_for_one: bool,
        sqrt_price_limit: U256,
        recipient: &solana_program::pubkey::Pubkey,
        price_history: &mut VecDeque<MarketDataPoint>,
        volume_history: &mut VecDeque<MarketDataPoint>,
        impact_history: &mut VecDeque<MarketDataPoint>,
        oracle_observations: &mut VecDeque<OracleObservation>,
        current_timestamp: u32,
        sequence_number: u64,
    ) -> Result<SwapResult, ProgramError> {
        if !pool.unlocked {
            return Err(CLMMError::Unauthorized.into());
        }

        // Calculate price impact
        let price_impact = Self::calculate_price_impact(pool, amount_in, zero_for_one)?;

        // Check price limit
        if !Self::validate_price_limit(pool.sqrt_price_x96, sqrt_price_limit, zero_for_one) {
            return Err(CLMMError::InvalidPrice.into());
        }

        // Validate transaction ordering for MEV protection
        if !MevProtectionEngine::validate_transaction_ordering(sequence_number, pool.last_sequence_number)? {
            return Err(CLMMError::InvalidInstruction.into());
        }

        // Validate swap against MEV protection measures
        if !MevProtectionEngine::validate_swap_mev_protection(
            pool,
            amount_in,
            zero_for_one,
            sqrt_price_limit,
            oracle_observations,
            &pool.mev_config,
        )? {
            return Err(CLMMError::InvalidPrice.into());
        }

        // Update dynamic fees based on market conditions
        let fee_adjusted = Self::update_dynamic_fees(
            pool,
            price_history,
            volume_history,
            impact_history,
            current_timestamp,
            pool.sqrt_price_x96,
            amount_in,
            price_impact,
        )?;

        let current_tick = pool.tick;
        let mut amount_out = U256_ZERO;
        let mut amount_in_used = U256_ZERO;

        // Execute the swap step by step
        while amount_in_used < amount_in {
            let remaining_amount = amount_in - amount_in_used;
            let step_result = Self::swap_step(pool, remaining_amount, zero_for_one)?;

            amount_in_used = amount_in_used + step_result.amount_in;
            amount_out = amount_out + step_result.amount_out;

            // Check if we've hit the price limit
            if Self::check_price_limit_hit(pool.sqrt_price_x96, sqrt_price_limit, zero_for_one) {
                break;
            }

            // Prevent infinite loops
            if step_result.amount_in == U256_ZERO {
                break;
            }
        }

        // Update pool state
        Self::update_pool_after_swap(pool, amount_in_used, amount_out, zero_for_one)?;

        // Update oracle observations and sequence number
        pool.last_sequence_number = sequence_number;
        MevProtectionEngine::update_oracle_observations(
            oracle_observations,
            pool,
            current_timestamp,
            100, // Max 100 oracle observations
        )?;

        // Calculate TWAP for result
        let twap_price = MevProtectionEngine::calculate_twap(oracle_observations, pool.mev_config.oracle_window)
            .unwrap_or(pool.sqrt_price_x96);

        Ok(SwapResult {
            amount_in: amount_in_used,
            amount_out,
            price_impact,
            final_sqrt_price: pool.sqrt_price_x96,
            final_tick: pool.tick,
            fee_adjusted,
            current_fee: pool.fee,
            mev_protected: true,
            twap_price,
        })
    }

    /// Single swap step for concentrated liquidity
    fn swap_step(
        pool: &mut Pool,
        amount_remaining: U256,
        zero_for_one: bool,
    ) -> Result<SwapStepResult, ProgramError> {
        let current_sqrt_price = pool.sqrt_price_x96;
        let current_tick = pool.tick;
        let current_liquidity = pool.liquidity;

        // Find the next tick to cross
        let (next_tick, next_sqrt_price) = if zero_for_one {
            // Swapping token0 for token1 (price decreases)
            Self::find_next_tick_down(pool, current_tick)?
        } else {
            // Swapping token1 for token0 (price increases)
            Self::find_next_tick_up(pool, current_tick)?
        };

        // Calculate maximum amount that can be swapped in this step
        let max_amount_in_step = Self::calculate_max_amount_in_step(
            current_sqrt_price,
            next_sqrt_price,
            current_liquidity,
            zero_for_one,
        )?;

        let amount_in_step = amount_remaining.min(max_amount_in_step);

        if amount_in_step == U256_ZERO {
            return Ok(SwapStepResult {
                amount_in: U256_ZERO,
                amount_out: U256_ZERO,
                sqrt_price_next: current_sqrt_price,
                tick_next: current_tick,
                liquidity_next: current_liquidity,
            });
        }

        // Calculate output amount
        let amount_out_step = if zero_for_one {
            FixedPointMath::get_amount1_delta(
                current_sqrt_price,
                next_sqrt_price,
                current_liquidity,
                false,
            )
        } else {
            FixedPointMath::get_amount0_delta(
                current_sqrt_price,
                next_sqrt_price,
                current_liquidity,
                false,
            )
        };

        // Update pool state
        let new_sqrt_price = Self::calculate_new_sqrt_price(
            current_sqrt_price,
            current_liquidity,
            amount_in_step,
            zero_for_one,
        )?;

        pool.sqrt_price_x96 = new_sqrt_price;
        pool.tick = Self::get_tick_at_sqrt_price(new_sqrt_price)?;

        Ok(SwapStepResult {
            amount_in: amount_in_step,
            amount_out: amount_out_step,
            sqrt_price_next: new_sqrt_price,
            tick_next: pool.tick,
            liquidity_next: current_liquidity,
        })
    }

    /// Calculate price impact of a swap
    pub fn calculate_price_impact(
        pool: &Pool,
        amount_in: U256,
        zero_for_one: bool,
    ) -> Result<u32, ProgramError> {
        if pool.liquidity == U256_ZERO {
            return Ok(10000); // 100% impact if no liquidity
        }

        let current_price = FixedPointMath::sqrt_price_x96_to_price(pool.sqrt_price_x96);
        let amount_out = Self::estimate_swap_output(pool, amount_in, zero_for_one)?;

        if amount_out == U256_ZERO {
            return Ok(10000); // 100% impact
        }

        let expected_price = if zero_for_one {
            current_price * ((amount_in.low_u128() as f64) / (amount_out.low_u128() as f64))
        } else {
            current_price * ((amount_out.low_u128() as f64) / (amount_in.low_u128() as f64))
        };

        let price_impact = ((expected_price - current_price).abs() / current_price) * 10000.0;
        Ok(price_impact.min(10000.0) as u32) // Cap at 100%
    }

    /// Estimate swap output without executing
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

        // Use current pool fee (may be dynamically adjusted)
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

    /// Validate price limit for swap
    fn validate_price_limit(
        current_price: U256,
        limit_price: U256,
        zero_for_one: bool,
    ) -> bool {
        if zero_for_one {
            // Price decreasing, limit should be >= current price
            limit_price >= current_price
        } else {
            // Price increasing, limit should be <= current price
            limit_price <= current_price
        }
    }

    /// Check if price limit is hit
    fn check_price_limit_hit(
        current_price: U256,
        limit_price: U256,
        zero_for_one: bool,
    ) -> bool {
        if zero_for_one {
            current_price <= limit_price
        } else {
            current_price >= limit_price
        }
    }

    /// Find next tick moving down (for zero_for_one swaps)
    fn find_next_tick_down(pool: &mut Pool, current_tick: i32) -> Result<(i32, U256), ProgramError> {
        // Simplified - in a real implementation, this would search the tick bitmap
        let next_tick = current_tick - pool.tick_spacing as i32;
        let next_sqrt_price = crate::math::TickMath::get_sqrt_ratio_at_tick(next_tick)?;

        Ok((next_tick, next_sqrt_price))
    }

    /// Find next tick moving up (for one_for_zero swaps)
    fn find_next_tick_up(pool: &mut Pool, current_tick: i32) -> Result<(i32, U256), ProgramError> {
        // Simplified - in a real implementation, this would search the tick bitmap
        let next_tick = current_tick + pool.tick_spacing as i32;
        let next_sqrt_price = crate::math::TickMath::get_sqrt_ratio_at_tick(next_tick)?;

        Ok((next_tick, next_sqrt_price))
    }

    /// Calculate maximum amount that can be swapped in this step
    fn calculate_max_amount_in_step(
        current_sqrt_price: U256,
        next_sqrt_price: U256,
        liquidity: U256,
        zero_for_one: bool,
    ) -> Result<U256, ProgramError> {
        if zero_for_one {
            Ok(FixedPointMath::get_amount0_delta(
                current_sqrt_price,
                next_sqrt_price,
                liquidity,
                false,
            ))
        } else {
            Ok(FixedPointMath::get_amount1_delta(
                current_sqrt_price,
                next_sqrt_price,
                liquidity,
                false,
            ))
        }
    }

    /// Calculate new sqrt price after swap
    fn calculate_new_sqrt_price(
        current_sqrt_price: U256,
        liquidity: U256,
        amount_in: U256,
        zero_for_one: bool,
    ) -> Result<U256, ProgramError> {
        if zero_for_one {
            crate::math::TickMath::get_next_sqrt_price_from_amount0_rounding_up(
                current_sqrt_price,
                liquidity,
                amount_in,
                true,
            )
        } else {
            crate::math::TickMath::get_next_sqrt_price_from_amount1_rounding_down(
                current_sqrt_price,
                liquidity,
                amount_in,
                false,
            )
        }
    }

    /// Get tick at sqrt price
    fn get_tick_at_sqrt_price(sqrt_price: U256) -> Result<i32, ProgramError> {
        crate::math::TickMath::get_tick_at_sqrt_ratio(sqrt_price)
    }

    /// Update pool state after swap
    fn update_pool_after_swap(
        pool: &mut Pool,
        amount_in: U256,
        amount_out: U256,
        zero_for_one: bool,
    ) -> Result<(), ProgramError> {
        // Update global fee growth using current pool fee
        let fee_amount = amount_in * U256::from(pool.fee) / U256::from(10000);
        let amount_after_fee = amount_in - fee_amount;

        if zero_for_one {
            // Fee on token0
            let fee_growth = fee_amount * Q96 / pool.liquidity;
            pool.fee_growth_global0_x128 = pool.fee_growth_global0_x128 + fee_growth;
        } else {
            // Fee on token1
            let fee_growth = fee_amount * Q96 / pool.liquidity;
            pool.fee_growth_global1_x128 = pool.fee_growth_global1_x128 + fee_growth;
        }

        pool.update_timestamp(chrono::Utc::now().timestamp() as u32);

        Ok(())
    }
}

/// Result of a swap operation
#[derive(Debug, Clone)]
pub struct SwapResult {
    pub amount_in: U256,
    pub amount_out: U256,
    pub price_impact: u32,
    pub final_sqrt_price: U256,
    pub final_tick: i32,
    pub fee_adjusted: bool,
    pub current_fee: u32,
    pub mev_protected: bool,
    pub twap_price: U256,
}

/// Result of a single swap step
#[derive(Debug)]
struct SwapStepResult {
    pub amount_in: U256,
    pub amount_out: U256,
    pub sqrt_price_next: U256,
    pub tick_next: i32,
    pub liquidity_next: U256,
}

