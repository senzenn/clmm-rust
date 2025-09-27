use borsh::{BorshDeserialize, BorshSerialize};
use crate::math::tick_math::{U256, I256};

/// Represents a tick in the concentrated liquidity system
#[derive(BorshDeserialize, BorshSerialize, Debug, Clone, PartialEq)]
pub struct Tick {
    /// The tick index
    pub tick: i32,

    /// Total liquidity at this tick (sum of all positions)
    pub liquidity_gross: U256,
    /// Net liquidity at this tick (can be negative)
    pub liquidity_net: I256,

    /// Fee growth outside this tick (token0)
    pub fee_growth_outside0_x128: U256,
    /// Fee growth outside this tick (token1)
    pub fee_growth_outside1_x128: U256,

    /// Cumulative tick value (for TWAP calculations)
    pub tick_cumulative_outside: I256,
    /// Cumulative seconds per liquidity outside (for TWAP calculations)
    pub seconds_per_liquidity_outside_x128: U256,
    /// Seconds outside this tick
    pub seconds_outside: u32,

    /// Whether this tick has been initialized
    pub initialized: bool,

    /// Reserve space for future fields
    pub reserved: [u8; 256],
}

impl Tick {
    /// Create a new uninitialized tick
    pub fn new(tick: i32) -> Self {
        Tick {
            tick,
            liquidity_gross: U256_ZERO,
            liquidity_net: I256_ZERO,
            fee_growth_outside0_x128: U256_ZERO,
            fee_growth_outside1_x128: U256_ZERO,
            tick_cumulative_outside: I256_ZERO,
            seconds_per_liquidity_outside_x128: U256_ZERO,
            seconds_outside: 0,
            initialized: false,
            reserved: [0; 256],
        }
    }

    /// Create a new initialized tick
    pub fn new_initialized(tick: i32) -> Self {
        Tick {
            tick,
            liquidity_gross: U256_ZERO,
            liquidity_net: I256_ZERO,
            fee_growth_outside0_x128: U256_ZERO,
            fee_growth_outside1_x128: U256_ZERO,
            tick_cumulative_outside: I256_ZERO,
            seconds_per_liquidity_outside_x128: U256_ZERO,
            seconds_outside: 0,
            initialized: true,
            reserved: [0; 256],
        }
    }

    /// Initialize the tick
    pub fn initialize(&mut self) {
        self.initialized = true;
    }

    /// Update liquidity at this tick
    pub fn update_liquidity(&mut self, liquidity_delta: I256, upper: bool) {
        if !self.initialized {
            self.initialize();
        }

        if upper {
            self.liquidity_net = self.liquidity_net + liquidity_delta;
            self.liquidity_gross = self.liquidity_gross + liquidity_delta.abs();
        } else {
            self.liquidity_net = self.liquidity_net - liquidity_delta;
            self.liquidity_gross = self.liquidity_gross + liquidity_delta.abs();
        }
    }

    /// Update fee growth outside this tick
    pub fn update_fee_growth_outside(
        &mut self,
        fee_growth_outside0_x128: U256,
        fee_growth_outside1_x128: U256,
    ) {
        if !self.initialized {
            self.initialize();
        }

        self.fee_growth_outside0_x128 = fee_growth_outside0_x128;
        self.fee_growth_outside1_x128 = fee_growth_outside1_x128;
    }

    /// Update cumulative values for TWAP calculations
    pub fn update_cumulative_values(
        &mut self,
        tick_cumulative: I256,
        seconds_per_liquidity_cumulative_x128: U256,
        seconds_outside: u32,
    ) {
        if !self.initialized {
            self.initialize();
        }

        self.tick_cumulative_outside = tick_cumulative;
        self.seconds_per_liquidity_outside_x128 = seconds_per_liquidity_cumulative_x128;
        self.seconds_outside = seconds_outside;
    }

    /// Check if this tick has liquidity
    pub fn has_liquidity(&self) -> bool {
        !self.liquidity_gross.is_zero()
    }

    /// Get the net liquidity change when crossing this tick
    pub fn cross(&self) -> I256 {
        self.liquidity_net
    }

    /// Check if the tick is valid (within bounds)
    pub fn is_valid(&self) -> bool {
        self.tick >= crate::math::tick_math::MIN_TICK && self.tick <= crate::math::tick_math::MAX_TICK
    }
}

/// Tick information for external use
#[derive(Debug, Clone)]
pub struct TickInfo {
    pub tick: i32,
    pub liquidity_gross: U256,
    pub liquidity_net: I256,
    pub fee_growth_outside0_x128: U256,
    pub fee_growth_outside1_x128: U256,
    pub tick_cumulative_outside: I256,
    pub seconds_per_liquidity_outside_x128: U256,
    pub seconds_outside: u32,
    pub initialized: bool,
}

impl From<&Tick> for TickInfo {
    fn from(tick: &Tick) -> Self {
        TickInfo {
            tick: tick.tick,
            liquidity_gross: tick.liquidity_gross,
            liquidity_net: tick.liquidity_net,
            fee_growth_outside0_x128: tick.fee_growth_outside0_x128,
            fee_growth_outside1_x128: tick.fee_growth_outside1_x128,
            tick_cumulative_outside: tick.tick_cumulative_outside,
            seconds_per_liquidity_outside_x128: tick.seconds_per_liquidity_outside_x128,
            seconds_outside: tick.seconds_outside,
            initialized: tick.initialized,
        }
    }
}

/// Tick bitmap for efficient tick tracking
#[derive(BorshDeserialize, BorshSerialize, Debug, Clone)]
pub struct TickBitmap {
    /// The bitmap data (each bit represents a tick)
    pub bitmap: [u8; 256],
    /// The word position (for larger bitmaps)
    pub word_position: i16,
}

impl TickBitmap {
    /// Create a new tick bitmap
    pub fn new(word_position: i16) -> Self {
        TickBitmap {
            bitmap: [0; 256],
            word_position,
        }
    }

    /// Set a bit at the given position
    pub fn set_bit(&mut self, bit_position: u8) {
        let byte_index = (bit_position / 8) as usize;
        let bit_index = (bit_position % 8) as u8;
        if byte_index < self.bitmap.len() {
            self.bitmap[byte_index] |= 1 << bit_index;
        }
    }

    /// Clear a bit at the given position
    pub fn clear_bit(&mut self, bit_position: u8) {
        let byte_index = (bit_position / 8) as usize;
        let bit_index = (bit_position % 8) as u8;
        if byte_index < self.bitmap.len() {
            self.bitmap[byte_index] &= !(1 << bit_index);
        }
    }

    /// Check if a bit is set at the given position
    pub fn is_bit_set(&self, bit_position: u8) -> bool {
        let byte_index = (bit_position / 8) as usize;
        let bit_index = (bit_position % 8) as u8;
        if byte_index < self.bitmap.len() {
            (self.bitmap[byte_index] & (1 << bit_index)) != 0
        } else {
            false
        }
    }

    /// Find the next initialized tick
    pub fn next_initialized_tick(&self, tick: i32, tick_spacing: u32, lte: bool) -> Option<i32> {
        let mut compressed = tick / tick_spacing as i32;
        if !lte && tick % tick_spacing as i32 != 0 {
            compressed += 1;
        }

        while compressed >= 0 {
            if self.is_bit_set(compressed as u8) {
                return Some(compressed * tick_spacing as i32);
            }
            if lte {
                compressed -= 1;
            } else {
                compressed += 1;
            }
        }

        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tick_creation() {
        let tick = Tick::new(100);
        assert_eq!(tick.tick, 100);
        assert!(!tick.initialized);
        assert!(tick.liquidity_gross.is_zero());
        assert!(tick.liquidity_net.is_zero());

        let tick_initialized = Tick::new_initialized(200);
        assert_eq!(tick_initialized.tick, 200);
        assert!(tick_initialized.initialized);
    }

    #[test]
    fn test_tick_liquidity_update() {
        let mut tick = Tick::new(100);

        tick.update_liquidity(I256([1000, 0, 0, 0]), true);
        assert!(tick.initialized);
        assert_eq!(tick.liquidity_net, I256([1000, 0, 0, 0]));
        assert_eq!(tick.liquidity_gross, U256([1000, 0, 0, 0]));

        tick.update_liquidity(I256([500, 0, 0, 0]), false);
        assert_eq!(tick.liquidity_net, I256([500, 0, 0, 0]));
        assert_eq!(tick.liquidity_gross, U256([1500, 0, 0, 0]));
    }

    #[test]
    fn test_tick_bitmap() {
        let mut bitmap = TickBitmap::new(0);

        bitmap.set_bit(5);
        assert!(bitmap.is_bit_set(5));
        assert!(!bitmap.is_bit_set(6));

        bitmap.clear_bit(5);
        assert!(!bitmap.is_bit_set(5));
    }

    #[test]
    fn test_tick_info() {
        let tick = Tick::new_initialized(100);
        let info: TickInfo = (&tick).into();

        assert_eq!(info.tick, 100);
        assert!(info.initialized);
        assert!(info.liquidity_gross.is_zero());
    }
}
