use borsh::{BorshDeserialize, BorshSerialize};
use solana_program::pubkey::Pubkey;
use crate::math::tick_math::U256;

/// Represents a liquidity position in a pool
#[derive(BorshDeserialize, BorshSerialize, Debug, Clone, PartialEq)]
pub struct Position {
    /// Pool this position belongs to
    pub pool_id: Pubkey,
    /// Owner of this position
    pub owner: Pubkey,
    /// Lower tick of the position range
    pub tick_lower: i32,
    /// Upper tick of the position range
    pub tick_upper: i32,

    /// Current liquidity amount in this position
    pub liquidity: U256,

    /// Fee growth per unit of liquidity as of the last update (token0)
    pub fee_growth_inside0_last_x128: U256,
    /// Fee growth per unit of liquidity as of the last update (token1)
    pub fee_growth_inside1_last_x128: U256,

    /// Tokens owed to the position owner (token0)
    pub tokens_owed0: U256,
    /// Tokens owed to the position owner (token1)
    pub tokens_owed1: U256,

    /// Position ID (auto-incremented)
    pub position_id: u64,

    /// Timestamp when this position was created
    pub created_at: u32,
    /// Timestamp when this position was last updated
    pub updated_at: u32,

    /// Whether this position is active
    pub is_active: bool,

    /// Reserve space for future fields
    pub reserved: [u8; 256],
}

impl Position {
    /// Create a new position
    pub fn new(
        pool_id: Pubkey,
        owner: Pubkey,
        tick_lower: i32,
        tick_upper: i32,
        position_id: u64,
        created_at: u32,
    ) -> Result<Self, &'static str> {
        if tick_lower >= tick_upper {
            return Err("Lower tick must be less than upper tick");
        }

        Ok(Position {
            pool_id,
            owner,
            tick_lower,
            tick_upper,
            liquidity: U256_ZERO,
            fee_growth_inside0_last_x128: U256_ZERO,
            fee_growth_inside1_last_x128: U256_ZERO,
            tokens_owed0: U256_ZERO,
            tokens_owed1: U256_ZERO,
            position_id,
            created_at,
            updated_at: created_at,
            is_active: true,
            reserved: [0; 256],
        })
    }

    /// Check if this position is valid
    pub fn is_valid(&self) -> bool {
        self.tick_lower < self.tick_upper && self.owner != Pubkey::default()
    }

    /// Update the position's liquidity
    pub fn update_liquidity(&mut self, new_liquidity: U256, timestamp: u32) {
        self.liquidity = new_liquidity;
        self.updated_at = timestamp;
    }

    /// Add tokens owed to the position
    pub fn add_tokens_owed(&mut self, token0_amount: U256, token1_amount: U256) {
        self.tokens_owed0 = self.tokens_owed0.saturating_add(token0_amount);
        self.tokens_owed1 = self.tokens_owed1.saturating_add(token1_amount);
    }

    /// Collect tokens owed (reduce the owed amounts)
    pub fn collect_tokens_owed(&mut self, token0_amount: U256, token1_amount: U256) -> (U256, U256) {
        let collected0 = self.tokens_owed0.min(token0_amount);
        let collected1 = self.tokens_owed1.min(token1_amount);

        self.tokens_owed0 = self.tokens_owed0.saturating_sub(collected0);
        self.tokens_owed1 = self.tokens_owed1.saturating_sub(collected1);

        (collected0, collected1)
    }

    /// Update fee growth tracking
    pub fn update_fee_growth(
        &mut self,
        fee_growth_inside0: U256,
        fee_growth_inside1: U256,
        timestamp: u32,
    ) {
        self.fee_growth_inside0_last_x128 = fee_growth_inside0;
        self.fee_growth_inside1_last_x128 = fee_growth_inside1;
        self.updated_at = timestamp;
    }

    /// Check if the position contains a given tick
    pub fn contains_tick(&self, tick: i32) -> bool {
        self.tick_lower <= tick && tick <= self.tick_upper
    }

    /// Get the tick range as a tuple
    pub fn tick_range(&self) -> (i32, i32) {
        (self.tick_lower, self.tick_upper)
    }

    /// Calculate the width of the position (in ticks)
    pub fn width(&self) -> u32 {
        (self.tick_upper - self.tick_lower) as u32
    }

    /// Check if the position is empty
    pub fn is_empty(&self) -> bool {
        self.liquidity == U256_ZERO
    }

    /// Deactivate the position
    pub fn deactivate(&mut self, timestamp: u32) {
        self.is_active = false;
        self.updated_at = timestamp;
    }
}

/// Information about a position for external use
#[derive(Debug, Clone)]
pub struct PositionInfo {
    pub position_id: u64,
    pub pool_id: Pubkey,
    pub owner: Pubkey,
    pub tick_lower: i32,
    pub tick_upper: i32,
    pub liquidity: U256,
    pub tokens_owed0: U256,
    pub tokens_owed1: U256,
    pub is_active: bool,
    pub created_at: u32,
    pub updated_at: u32,
}

impl From<&Position> for PositionInfo {
    fn from(position: &Position) -> Self {
        PositionInfo {
            position_id: position.position_id,
            pool_id: position.pool_id,
            owner: position.owner,
            tick_lower: position.tick_lower,
            tick_upper: position.tick_upper,
            liquidity: position.liquidity,
            tokens_owed0: position.tokens_owed0,
            tokens_owed1: position.tokens_owed1,
            is_active: position.is_active,
            created_at: position.created_at,
            updated_at: position.updated_at,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_position_creation() {
        let pool_id = Pubkey::new_unique();
        let owner = Pubkey::new_unique();
        let timestamp = 1000u32;

        let position = Position::new(
            pool_id,
            owner,
            -100,
            100,
            1,
            timestamp,
        ).unwrap();

        assert!(position.is_valid());
        assert_eq!(position.tick_lower, -100);
        assert_eq!(position.tick_upper, 100);
        assert_eq!(position.position_id, 1);
        assert_eq!(position.created_at, timestamp);
        assert!(position.is_active);
    }

    #[test]
    fn test_position_validation() {
        let pool_id = Pubkey::new_unique();
        let owner = Pubkey::new_unique();

        assert!(Position::new(pool_id, owner, 100, 100, 1, 1000).is_err());

        let position = Position {
            pool_id,
            owner: Pubkey::default(),
            tick_lower: -100,
            tick_upper: 100,
            liquidity: U256_ZERO,
            fee_growth_inside0_last_x128: U256_ZERO,
            fee_growth_inside1_last_x128: U256_ZERO,
            tokens_owed0: U256_ZERO,
            tokens_owed1: U256_ZERO,
            position_id: 1,
            created_at: 1000,
            updated_at: 1000,
            is_active: true,
            reserved: [0; 256],
        };

        assert!(!position.is_valid());
    }

    #[test]
    fn test_position_operations() {
        let pool_id = Pubkey::new_unique();
        let owner = Pubkey::new_unique();
        let mut position = Position::new(
            pool_id,
            owner,
            -100,
            100,
            1,
            1000,
        ).unwrap();

        let new_liquidity = U256([1000, 0, 0, 0]);
        position.update_liquidity(new_liquidity, 2000);
        assert_eq!(position.liquidity, new_liquidity);
        assert_eq!(position.updated_at, 2000);

        position.add_tokens_owed(U256([100, 0, 0, 0]), U256([200, 0, 0, 0]));
        assert_eq!(position.tokens_owed0, U256([100, 0, 0, 0]));
        assert_eq!(position.tokens_owed1, U256([200, 0, 0, 0]));

        let (collected0, collected1) = position.collect_tokens_owed(
            U256([50, 0, 0, 0]),
            U256([150, 0, 0, 0]),
        );
        assert_eq!(collected0, U256([50, 0, 0, 0]));
        assert_eq!(collected1, U256([150, 0, 0, 0]));
        assert_eq!(position.tokens_owed0, U256([50, 0, 0, 0]));
        assert_eq!(position.tokens_owed1, U256([50, 0, 0, 0]));
    }

    #[test]
    fn test_position_info() {
        let pool_id = Pubkey::new_unique();
        let owner = Pubkey::new_unique();
        let position = Position::new(
            pool_id,
            owner,
            -100,
            100,
            1,
            1000,
        ).unwrap();

        let info: PositionInfo = (&position).into();
        assert_eq!(info.position_id, 1);
        assert_eq!(info.pool_id, pool_id);
        assert_eq!(info.owner, owner);
        assert_eq!(info.tick_lower, -100);
        assert_eq!(info.tick_upper, 100);
        assert!(info.is_active);
    }
}
