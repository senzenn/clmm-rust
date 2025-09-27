use borsh::{BorshDeserialize, BorshSerialize};
use solana_program::pubkey::Pubkey;
use crate::math::tick_math::{U256, I256};

/// Represents a concentrated liquidity pool
#[derive(BorshDeserialize, BorshSerialize, Debug, Clone, PartialEq)]
pub struct Pool {
    /// Address of token A
    pub token_a: Pubkey,
    /// Address of token B
    pub token_b: Pubkey,
    /// Fee tier of the pool (in basis points)
    pub fee: u32,
    /// Tick spacing for this fee tier
    pub tick_spacing: u32,
    /// Maximum liquidity per tick
    pub max_liquidity_per_tick: U256,

    /// Current sqrt price of the pool (X96 format)
    pub sqrt_price_x96: U256,
    /// Current tick of the pool
    pub tick: i32,

    /// Global fee growth for token0 as of the last update
    pub fee_growth_global0_x128: U256,
    /// Global fee growth for token1 as of the last update
    pub fee_growth_global1_x128: U256,

    /// Protocol fees accumulated in token0
    pub protocol_fees_token0: U256,
    /// Protocol fees accumulated in token1
    pub protocol_fees_token1: U256,

    /// Total liquidity in the pool
    pub liquidity: U256,

    /// Number of positions in this pool
    pub position_count: u64,

    /// Timestamp of the last update
    pub last_update_timestamp: u32,

    /// Whether the pool is unlocked (for reentrancy protection)
    pub unlocked: bool,

    /// Reserve space for future fields
    pub reserved: [u8; 256],
}

impl Pool {
    /// Create a new pool with initial parameters
    pub fn new(
        token_a: Pubkey,
        token_b: Pubkey,
        fee: u32,
        tick_spacing: u32,
        initial_sqrt_price_x96: U256,
    ) -> Result<Self, &'static str> {
        let (token_a, token_b) = if token_a < token_b {
            (token_a, token_b)
        } else {
            (token_b, token_a)
        };

        let initial_tick = crate::math::TickMath::get_tick_at_sqrt_ratio(initial_sqrt_price_x96)
            .map_err(|_| "Invalid initial sqrt price")?;

        Ok(Pool {
            token_a,
            token_b,
            fee,
            tick_spacing,
            max_liquidity_per_tick: U256::MAX,
            sqrt_price_x96: initial_sqrt_price_x96,
            tick: initial_tick,
            fee_growth_global0_x128: U256::zero(),
            fee_growth_global1_x128: U256::zero(),
            protocol_fees_token0: U256::zero(),
            protocol_fees_token1: U256::zero(),
            liquidity: U256::zero(),
            position_count: 0,
            last_update_timestamp: 0,
            unlocked: true,
            reserved: [0; 256],
        })
    }

    /// Check if the pool is valid (tokens sorted, fee in range)
    pub fn is_valid(&self) -> bool {
        self.token_a < self.token_b && self.fee <= 10000 && self.tick_spacing > 0
    }

    /// Get the current price as a float
    pub fn price(&self) -> f64 {
        crate::math::FixedPointMath::sqrt_price_x96_to_price(self.sqrt_price_x96)
    }

    /// Update the pool's timestamp
    pub fn update_timestamp(&mut self, timestamp: u32) {
        self.last_update_timestamp = timestamp;
    }

    /// Check if a tick is properly spaced for this pool
    pub fn is_tick_spacing_valid(&self, tick: i32) -> bool {
        tick % self.tick_spacing as i32 == 0
    }

    /// Get the minimum tick for this pool
    pub fn min_tick(&self) -> i32 {
        (crate::math::tick_math::MIN_TICK / self.tick_spacing as i32) * self.tick_spacing as i32
    }

    /// Get the maximum tick for this pool
    pub fn max_tick(&self) -> i32 {
        (crate::math::tick_math::MAX_TICK / self.tick_spacing as i32) * self.tick_spacing as i32
    }

    /// Validate tick range for this pool
    pub fn validate_tick_range(&self, tick_lower: i32, tick_upper: i32) -> Result<(), &'static str> {
        if !self.is_tick_spacing_valid(tick_lower) {
            return Err("Lower tick not properly spaced");
        }
        if !self.is_tick_spacing_valid(tick_upper) {
            return Err("Upper tick not properly spaced");
        }
        if tick_lower >= tick_upper {
            return Err("Lower tick must be less than upper tick");
        }
        if tick_lower < self.min_tick() {
            return Err("Lower tick below minimum");
        }
        if tick_upper > self.max_tick() {
            return Err("Upper tick above maximum");
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pool_creation() {
        let token_a = Pubkey::new_unique();
        let token_b = Pubkey::new_unique();
        let initial_price = U256::from(1000000000000000000000000u128);

        let pool = Pool::new(token_a, token_b, 300, 60, initial_price).unwrap();
        assert!(pool.is_valid());
        assert_eq!(pool.fee, 300);
        assert_eq!(pool.tick_spacing, 60);
    }

    #[test]
    fn test_token_sorting() {
        let token_a = Pubkey::new_unique();
        let token_b = Pubkey::new_unique();
        let initial_price = U256::from(1000000000000000000000000u128);

        let pool = Pool::new(token_b, token_a, 300, 60, initial_price).unwrap();
        assert!(pool.token_a < pool.token_b);
    }

    #[test]
    fn test_tick_validation() {
        let token_a = Pubkey::new_unique();
        let token_b = Pubkey::new_unique();
        let initial_price = U256::from(1000000000000000000000000u128);

        let pool = Pool::new(token_a, token_b, 300, 60, initial_price).unwrap();

        assert!(pool.validate_tick_range(-60, 60).is_ok());

        assert!(pool.validate_tick_range(60, 60).is_err());

        assert!(pool.validate_tick_range(-50, 60).is_err());
    }
}
