use crate::error::CLMMError;
use solana_program::program_error::ProgramError;
use uint::construct_uint;

/// 256-bit unsigned integer for precise calculations
pub type U256 = construct_uint! {
    pub struct U256(4);
};

/// 256-bit signed integer for tick calculations
pub type I256 = construct_uint! {
    pub struct I256(4);
};

pub const MIN_TICK: i32 = -887272;
pub const MAX_TICK: i32 = 887272;
pub const Q96: U256 = U256([0, 0, 0, 1 << 32]);

pub struct TickMath;

impl TickMath {
    /// Calculate the sqrt price ratio at a given tick
    pub fn get_sqrt_ratio_at_tick(tick: i32) -> Result<U256, ProgramError> {
        if tick < MIN_TICK || tick > MAX_TICK {
            return Err(CLMMError::InvalidTickRange.into());
        }

        let abs_tick = if tick < 0 {
            U256::from(tick.abs() as u64)
        } else {
            U256::from(tick as u64)
        };

        let mut ratio = if tick % 2 == 0 {
            U256::from(0xfffcb933bd6fad37aa2d162d1a594001_u128)
        } else {
            U256::from(0xfff97272373d413259a46990580e213a_u128)
        };

        if tick & 0x02 != 0 {
            ratio = (ratio * U256::from(0xfff2e50f5f656932ef12357cf3c7fdcc_u128)) >> 128;
        }
        if tick & 0x04 != 0 {
            ratio = (ratio * U256::from(0xffe5caca7e10e4e61c3624eaa0941cd0_u128)) >> 128;
        }
        if tick & 0x08 != 0 {
            ratio = (ratio * U256::from(0xffcb9843d60f6159c9db58835c926644_u128)) >> 128;
        }
        if tick & 0x10 != 0 {
            ratio = (ratio * U256::from(0xff973b41fa98c081472e6896dfb254c0_u128)) >> 128;
        }
        if tick & 0x20 != 0 {
            ratio = (ratio * U256::from(0xff2ea16466c96a3843ec78b326b52861_u128)) >> 128;
        }
        if tick & 0x40 != 0 {
            ratio = (ratio * U256::from(0xfe5dee046a99a2a811c461f1969c3053_u128)) >> 128;
        }
        if tick & 0x80 != 0 {
            ratio = (ratio * U256::from(0xfcbe86c7900a88aedcffc83b479aa3a4_u128)) >> 128;
        }
        if tick & 0x100 != 0 {
            ratio = (ratio * U256::from(0xf987a7253acae65be8623aa479a2ddf0_u128)) >> 128;
        }
        if tick & 0x200 != 0 {
            ratio = (ratio * U256::from(0xf3392b0822b70005940c7a398e4b70f3_u128)) >> 128;
        }
        if tick & 0x400 != 0 {
            ratio = (ratio * U256::from(0xe7159475a2c29be046d0ccceb0512d9_u128)) >> 128;
        }
        if tick & 0x800 != 0 {
            ratio = (ratio * U256::from(0xd097f3bdfd2022b8845ad8f792aa5825_u128)) >> 128;
        }
        if tick & 0x1000 != 0 {
            ratio = (ratio * U256::from(0xa9f746462d870fdf8a65dc1f90e061e5_u128)) >> 128;
        }
        if tick & 0x2000 != 0 {
            ratio = (ratio * U256::from(0x70d869a156d2a1b890bb3df62baf32f7_u128)) >> 128;
        }
        if tick & 0x4000 != 0 {
            ratio = (ratio * U256::from(0x31be135f97d08fd981231505542fcfa6_u128)) >> 128;
        }
        if tick & 0x8000 != 0 {
            ratio = (ratio * U256::from(0x9aa508b5b7a84e1c677de54f3e99bc9_u128)) >> 128;
        }
        if tick & 0x10000 != 0 {
            ratio = (ratio * U256::from(0x5d6af8dedb81196699c329225ee604_u128)) >> 128;
        }
        if tick & 0x20000 != 0 {
            ratio = (ratio * U256::from(0x2216e584f5fa1ea926041bedfe98_u128)) >> 128;
        }
        if tick & 0x40000 != 0 {
            ratio = (ratio * U256::from(0x48a170391f7dc42444e8fa2_u128)) >> 128;
        }

        if tick > 0 {
            ratio = U256::MAX / ratio;
        }

        Ok(ratio)
    }

    /// Get the tick at a given sqrt price ratio
    pub fn get_tick_at_sqrt_ratio(sqrt_price_x96: U256) -> Result<i32, ProgramError> {
        if sqrt_price_x96 < U256::from(4295128739u64) || sqrt_price_x96 >= U256::from(1461446703485210103287273052203988822378723970342u128) {
            return Err(CLMMError::InvalidPrice.into());
        }

        let ratio = sqrt_price_x96;
        let mut r = ratio;
        let mut msb = 0u8;

        // Binary search for most significant bit
        let mut f = if r > U256::from(0xFFFFFFFFFFFFFFFFFFFFFFFFu128) { 1u8 } else { 0u8 } << 7;
        msb |= f;
        r >>= f;

        f = if r > U256::from(0xFFFFFFFFFFFFFFFFu64) { 1u8 } else { 0u8 } << 6;
        msb |= f;
        r >>= f;

        f = if r > U256::from(0xFFFFFFFFu32) { 1u8 } else { 0u8 } << 5;
        msb |= f;
        r >>= f;

        f = if r > U256::from(0xFFFFu16) { 1u8 } else { 0u8 } << 4;
        msb |= f;
        r >>= f;

        f = if r > U256::from(0xFFu8) { 1u8 } else { 0u8 } << 3;
        msb |= f;
        r >>= f;

        f = if r > U256::from(0xF) { 1u8 } else { 0u8 } << 2;
        msb |= f;
        r >>= f;

        f = if r > U256::from(0x3) { 1u8 } else { 0u8 } << 1;
        msb |= f;
        r >>= f;

        f = if r > U256::from(0x1) { 1u8 } else { 0u8 };
        msb |= f;

        let log_2 = (U256::from(msb) - U256::from(64)) << 64;

        let mut r2 = (sqrt_price_x96 * sqrt_price_x96) >> 128;
        r2 = (r2 * sqrt_price_x96) >> 128;

        let tick_low = (log_2 - U256::from(0x100000000000000000000000000000000u128)) >> 128;
        let tick_high = (log_2 + U256::from(0x100000000000000000000000000000000u128)) >> 128;

        let tick = if tick_low == tick_high {
            tick_low.to::<i32>().unwrap_or(0)
        } else if Self::get_sqrt_ratio_at_tick(tick_low.to::<i32>().unwrap_or(0))? <= sqrt_price_x96 {
            tick_low.to::<i32>().unwrap_or(0)
        } else {
            tick_high.to::<i32>().unwrap_or(0)
        };

        Ok(tick)
    }

    /// Calculate the next sqrt price from amount0 rounding up
    pub fn get_next_sqrt_price_from_amount0_rounding_up(
        sqrt_px96: U256,
        liquidity: U256,
        amount: U256,
        add: bool,
    ) -> Result<U256, ProgramError> {
        if liquidity == U256::zero() {
            return Err(CLMMError::InsufficientLiquidity.into());
        }

        let numerator1 = liquidity << 96;
        if add {
            let liquidity_after = liquidity.checked_add(
                Self::mul_div_rounding_up(amount, Q96, sqrt_px96)?
            ).ok_or(CLMMError::MathOverflow)?;
            if liquidity_after == liquidity {
                return Err(CLMMError::MathOverflow.into());
            }
            Ok(Self::mul_div_rounding_up(numerator1, sqrt_px96, liquidity_after)?)
        } else {
            let liquidity_after = liquidity.checked_sub(
                Self::mul_div_rounding_up(amount, Q96, sqrt_px96)?
            ).ok_or(CLMMError::InsufficientLiquidity)?;
            Ok(numerator1 * sqrt_px96 / liquidity_after)
        }
    }

    /// Calculate the next sqrt price from amount1 rounding down
    pub fn get_next_sqrt_price_from_amount1_rounding_down(
        sqrt_px96: U256,
        liquidity: U256,
        amount: U256,
        add: bool,
    ) -> Result<U256, ProgramError> {
        if add {
            let liquidity_after = liquidity.checked_add(amount.shl(96) / sqrt_px96)
                .ok_or(CLMMError::MathOverflow)?;
            Ok(liquidity_after * sqrt_px96 / Q96)
        } else {
            let liquidity_after = liquidity.checked_sub(amount.shl(96) / sqrt_px96)
                .ok_or(CLMMError::InsufficientLiquidity)?;
            Ok(liquidity_after * sqrt_px96 / Q96)
        }
    }

    /// Multiply and divide with rounding up
    pub fn mul_div_rounding_up(a: U256, b: U256, denominator: U256) -> Result<U256, ProgramError> {
        let result = Self::mul_div(a, b, denominator)?;
        if a * b % denominator != U256::zero() {
            Ok(result + U256::one())
        } else {
            Ok(result)
        }
    }

    /// Multiply and divide
    pub fn mul_div(a: U256, b: U256, denominator: U256) -> Result<U256, ProgramError> {
        if denominator == U256::zero() {
            return Err(CLMMError::MathOverflow.into());
        }

        let a_low = a.low_u128() as u64;
        let b_low = b.low_u128() as u64;
        let denominator_low = denominator.low_u128() as u64;

        let a_high = (a >> 128).low_u128() as u64;
        let b_high = (b >> 128).low_u128() as u64;
        let denominator_high = (denominator >> 128).low_u128() as u64;

        let bd = b_low * denominator_low;
        let bn = b_high * denominator_low;
        let ad = a_low * denominator_high;
        let an = a_high * denominator_high;

        let mut result = a_low * b_low;
        let mut carry = if result > U256::MAX.low_u128() { 1u64 } else { 0u64 };

        result += carry << 64;

        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_sqrt_ratio_at_tick() {
        let ratio = TickMath::get_sqrt_ratio_at_tick(0).unwrap();
        assert!(ratio > U256::zero());

        let ratio_min = TickMath::get_sqrt_ratio_at_tick(MIN_TICK).unwrap();
        let ratio_max = TickMath::get_sqrt_ratio_at_tick(MAX_TICK).unwrap();
        assert!(ratio_max > ratio_min);
    }

    #[test]
    fn test_tick_math_bounds() {
        assert!(TickMath::get_sqrt_ratio_at_tick(MIN_TICK - 1).is_err());
        assert!(TickMath::get_sqrt_ratio_at_tick(MAX_TICK + 1).is_err());
    }
}
