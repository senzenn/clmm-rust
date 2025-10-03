use borsh::{BorshDeserialize, BorshSerialize};
use solana_program::pubkey::Pubkey;
use crate::math::tick_math::{U256, U256_ZERO, Uint256};
use crate::math::fixed_point::FixedPointMath;
use std::io::{Error, ErrorKind};

// Custom serialization for U256
impl BorshSerialize for U256 {
    fn serialize<W: std::io::Write>(&self, writer: &mut W) -> std::io::Result<()> {
        for i in 0..4 {
            self.0[i].serialize(writer)?;
        }
        Ok(())
    }
}

impl BorshDeserialize for U256 {
    fn deserialize(buf: &mut &[u8]) -> std::io::Result<Self> {
        let mut arr = [0u64; 4];
        for i in 0..4 {
            arr[i] = u64::deserialize(buf)?;
        }
        Ok(Uint256(arr))
    }

    fn deserialize_reader<R: std::io::Read>(reader: &mut R) -> std::io::Result<Self> {
        let mut arr = [0u64; 4];
        for i in 0..4 {
            arr[i] = u64::deserialize_reader(reader)?;
        }
        Ok(Uint256(arr))
    }
}

// a concentrated liquidity pool
#[derive(Debug, Clone, PartialEq)]
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

    /// Dynamic fee adjustment fields
    /// Base fee for dynamic adjustment (in basis points)
    pub base_fee: u32,
    /// Minimum allowed fee (in basis points)
    pub min_fee: u32,
    /// Maximum allowed fee (in basis points)
    pub max_fee: u32,
    /// Timestamp of last fee adjustment
    pub last_fee_adjustment: u32,
    /// Fee adjustment interval in seconds
    pub fee_adjustment_interval: u32,
    /// Whether dynamic fee adjustment is enabled
    pub dynamic_fee_enabled: bool,

    /// MEV Protection fields
    /// Timestamp of last oracle update
    pub last_oracle_update: u32,
    /// Oracle observation count
    pub oracle_observation_count: u32,
    /// Last processed sequence number for transaction ordering
    pub last_sequence_number: u64,
    /// Timestamp of last position update (for frequency limits)
    pub last_position_update: u32,
    /// MEV protection configuration
    pub mev_config: crate::math::mev_protection::MevConfig,

    /// Reserve space for future fields
    pub reserved: [u8; 200],
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
            fee_growth_global0_x128: U256_ZERO,
            fee_growth_global1_x128: U256_ZERO,
            protocol_fees_token0: U256_ZERO,
            protocol_fees_token1: U256_ZERO,
            liquidity: U256_ZERO,
            position_count: 0,
            last_update_timestamp: 0,
            unlocked: true,
            base_fee: fee,
            min_fee: 1, // 0.01%
            max_fee: 100, // 1.00%
            last_fee_adjustment: 0,
            fee_adjustment_interval: 3600, // 1 hour
            dynamic_fee_enabled: true,
            last_oracle_update: 0,
            oracle_observation_count: 0,
            last_sequence_number: 0,
            last_position_update: 0,
            mev_config: crate::math::mev_protection::MevProtectionEngine::default_config(),
            reserved: [0; 200],
        })
    }

    /// Check if the pool is valid (tokens sorted, fee in range)
    pub fn is_valid(&self) -> bool {
        self.token_a < self.token_b && self.fee <= 10000 && self.tick_spacing > 0
    }

    /// Get the current price as a float
    pub fn price(&self) -> f64 {
        FixedPointMath::sqrt_price_x96_to_price(self.sqrt_price_x96)
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

impl borsh::BorshSerialize for Pool {
    fn serialize<W: std::io::Write>(&self, writer: &mut W) -> std::io::Result<()> {
        self.token_a.serialize(writer)?;
        self.token_b.serialize(writer)?;
        self.fee.serialize(writer)?;
        self.tick_spacing.serialize(writer)?;
        self.max_liquidity_per_tick.serialize(writer)?;
        self.sqrt_price_x96.serialize(writer)?;
        self.tick.serialize(writer)?;
        self.fee_growth_global0_x128.serialize(writer)?;
        self.fee_growth_global1_x128.serialize(writer)?;
        self.protocol_fees_token0.serialize(writer)?;
        self.protocol_fees_token1.serialize(writer)?;
        self.liquidity.serialize(writer)?;
        self.position_count.serialize(writer)?;
        self.last_update_timestamp.serialize(writer)?;
        self.unlocked.serialize(writer)?;
        self.base_fee.serialize(writer)?;
        self.min_fee.serialize(writer)?;
        self.max_fee.serialize(writer)?;
        self.last_fee_adjustment.serialize(writer)?;
        self.fee_adjustment_interval.serialize(writer)?;
        self.dynamic_fee_enabled.serialize(writer)?;
        self.last_oracle_update.serialize(writer)?;
        self.oracle_observation_count.serialize(writer)?;
        self.last_sequence_number.serialize(writer)?;
        self.last_position_update.serialize(writer)?;
        self.mev_config.serialize(writer)?;
        self.reserved.serialize(writer)?;
        Ok(())
    }
}

impl borsh::BorshDeserialize for Pool {
    fn deserialize(buf: &mut &[u8]) -> std::io::Result<Self> {
        let token_a = Pubkey::deserialize(buf)?;
        let token_b = Pubkey::deserialize(buf)?;
        let fee = u32::deserialize(buf)?;
        let tick_spacing = u32::deserialize(buf)?;
        let max_liquidity_per_tick = U256::deserialize(buf)?;
        let sqrt_price_x96 = U256::deserialize(buf)?;
        let tick = i32::deserialize(buf)?;
        let fee_growth_global0_x128 = U256::deserialize(buf)?;
        let fee_growth_global1_x128 = U256::deserialize(buf)?;
        let protocol_fees_token0 = U256::deserialize(buf)?;
        let protocol_fees_token1 = U256::deserialize(buf)?;
        let liquidity = U256::deserialize(buf)?;
        let position_count = u64::deserialize(buf)?;
        let last_update_timestamp = u32::deserialize(buf)?;
        let unlocked = bool::deserialize(buf)?;
        let base_fee = u32::deserialize(buf)?;
        let min_fee = u32::deserialize(buf)?;
        let max_fee = u32::deserialize(buf)?;
        let last_fee_adjustment = u32::deserialize(buf)?;
        let fee_adjustment_interval = u32::deserialize(buf)?;
        let dynamic_fee_enabled = bool::deserialize(buf)?;
        let last_oracle_update_val = u32::deserialize(buf)?;
        let oracle_observation_count_val = u32::deserialize(buf)?;
        let last_sequence_number_val = u64::deserialize(buf)?;
        let last_position_update_val = u32::deserialize(buf)?;
        let mev_config_val = crate::math::mev_protection::MevConfig::deserialize(buf)?;
        let mut reserved = [0u8; 200];
        for i in 0..200 {
            reserved[i] = u8::deserialize(buf)?;
        }

        Ok(Pool {
            token_a,
            token_b,
            fee,
            tick_spacing,
            max_liquidity_per_tick,
            sqrt_price_x96,
            tick,
            fee_growth_global0_x128,
            fee_growth_global1_x128,
            protocol_fees_token0,
            protocol_fees_token1,
            liquidity,
            position_count,
            last_update_timestamp,
            unlocked,
            base_fee,
            min_fee,
            max_fee,
            last_fee_adjustment,
            fee_adjustment_interval,
            dynamic_fee_enabled,
            last_oracle_update: last_oracle_update_val,
            oracle_observation_count: oracle_observation_count_val,
            last_sequence_number: last_sequence_number_val,
            last_position_update: last_position_update_val,
            mev_config: mev_config_val,
            reserved: reserved,
        })
    }

    fn deserialize_reader<R: std::io::Read>(reader: &mut R) -> std::io::Result<Self> {
        let token_a = Pubkey::deserialize_reader(reader)?;
        let token_b = Pubkey::deserialize_reader(reader)?;
        let fee = u32::deserialize_reader(reader)?;
        let tick_spacing = u32::deserialize_reader(reader)?;
        let max_liquidity_per_tick = U256::deserialize_reader(reader)?;
        let sqrt_price_x96 = U256::deserialize_reader(reader)?;
        let tick = i32::deserialize_reader(reader)?;
        let fee_growth_global0_x128 = U256::deserialize_reader(reader)?;
        let fee_growth_global1_x128 = U256::deserialize_reader(reader)?;
        let protocol_fees_token0 = U256::deserialize_reader(reader)?;
        let protocol_fees_token1 = U256::deserialize_reader(reader)?;
        let liquidity = U256::deserialize_reader(reader)?;
        let position_count = u64::deserialize_reader(reader)?;
        let last_update_timestamp = u32::deserialize_reader(reader)?;
        let unlocked = bool::deserialize_reader(reader)?;
        let base_fee = u32::deserialize_reader(reader)?;
        let min_fee = u32::deserialize_reader(reader)?;
        let max_fee = u32::deserialize_reader(reader)?;
        let last_fee_adjustment = u32::deserialize_reader(reader)?;
        let fee_adjustment_interval = u32::deserialize_reader(reader)?;
        let dynamic_fee_enabled = bool::deserialize_reader(reader)?;
        let last_oracle_update_val = u32::deserialize_reader(reader)?;
        let oracle_observation_count_val = u32::deserialize_reader(reader)?;
        let last_sequence_number_val = u64::deserialize_reader(reader)?;
        let last_position_update_val = u32::deserialize_reader(reader)?;
        let mev_config_val = crate::math::mev_protection::MevConfig::deserialize_reader(reader)?;
        let mut reserved = [0u8; 200];
        reader.read_exact(&mut reserved)?;

        Ok(Pool {
            token_a,
            token_b,
            fee,
            tick_spacing,
            max_liquidity_per_tick,
            sqrt_price_x96,
            tick,
            fee_growth_global0_x128,
            fee_growth_global1_x128,
            protocol_fees_token0,
            protocol_fees_token1,
            liquidity,
            position_count,
            last_update_timestamp,
            unlocked,
            base_fee,
            min_fee,
            max_fee,
            last_fee_adjustment,
            fee_adjustment_interval,
            dynamic_fee_enabled,
            last_oracle_update: last_oracle_update_val,
            oracle_observation_count: oracle_observation_count_val,
            last_sequence_number: last_sequence_number_val,
            last_position_update: last_position_update_val,
            mev_config: mev_config_val,
            reserved: reserved,
        })
    }
}

