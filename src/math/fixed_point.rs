use crate::error::CLMMError;
use crate::math::tick_math::{U256, Q96, U256_ZERO, U256_ONE};
use solana_program::program_error::ProgramError;

pub struct FixedPointMath;

impl FixedPointMath {
    /// Multiply two U256 numbers and divide by a denominator with rounding up
    pub fn mul_div_rounding_up(x: U256, y: U256, denominator: U256) -> Result<U256, ProgramError> {
        let result = Self::mul_div(x, y, denominator)?;
        if x * y % denominator != U256_ZERO {
            Ok(result + U256_ONE)
        } else {
            Ok(result)
        }
    }

    /// Multiply two U256 numbers and divide by a denominator
    pub fn mul_div(x: U256, y: U256, denominator: U256) -> Result<U256, ProgramError> {
        if denominator == U256_ZERO {
            return Err(CLMMError::MathOverflow.into());
        }

        let x_u128 = x.low_u128();
        let y_u128 = y.low_u128();
        let denominator_u128 = denominator.low_u128();

        let result_low = x_u128 * y_u128 / denominator_u128;

        let x_high = (x >> 128).low_u128();
        let y_high = (y >> 128).low_u128();
        let denominator_high = (denominator >> 128).low_u128();

        let cross_term1 = x_u128 * y_high / denominator_u128;
        let cross_term2 = x_high * y_u128 / denominator_u128;
        let cross_term3 = x_high * y_high / denominator_high;

        let result_high = cross_term1 + cross_term2 + cross_term3;

        let mut result = U256::from(result_low);
        result |= U256::from(result_high) << 128;

        Ok(result)
    }

    pub fn sqrt(x: U256) -> Result<U256, ProgramError> {
        if x == U256_ZERO {
            return Ok(U256_ZERO);
        }

        let mut z = x;
        let mut y = (x + U256_ONE) >> 1;

        while y < z {
            z = y;
            y = (x / y + y) >> 1;
        }

        Ok(z)
    }

    pub fn get_amount0_for_liquidity(sqrt_a: U256, sqrt_b: U256, liquidity: U256) -> U256 {
        if sqrt_a > sqrt_b {
            Self::get_amount0_for_liquidity(sqrt_b, sqrt_a, liquidity)
        } else {
            (Self::mul_div(sqrt_a, sqrt_b, Q96).unwrap_or(U256_ZERO)) * liquidity / Q96
        }
    }

    /// Get amount1 for given liquidity and price range
    pub fn get_amount1_for_liquidity(sqrt_a: U256, sqrt_b: U256, liquidity: U256) -> U256 {
        if sqrt_a > sqrt_b {
            Self::get_amount1_for_liquidity(sqrt_b, sqrt_a, liquidity)
        } else {
            (sqrt_b - sqrt_a) * liquidity / Q96
        }
    }

    /// Calculate amount0 delta for a swap
    pub fn get_amount0_delta(
        sqrt_price_a: U256,
        sqrt_price_b: U256,
        liquidity: U256,
        round_up: bool,
    ) -> U256 {
        let (sqrt_price_start, sqrt_price_end) = if sqrt_price_a < sqrt_price_b {
            (sqrt_price_a, sqrt_price_b)
        } else {
            (sqrt_price_b, sqrt_price_a)
        };

        let numerator1 = liquidity << 96;
        let numerator2 = sqrt_price_end - sqrt_price_start;

        let amount0 =
            Self::div_rounding_up(numerator1 * numerator2, sqrt_price_end * sqrt_price_start);

        if round_up && (numerator1 * numerator2 % (sqrt_price_end * sqrt_price_start) != U256_ZERO)
        {
            amount0 + U256_ONE
        } else {
            amount0
        }
    }

    /// Calculate amount1 delta for a swap
    pub fn get_amount1_delta(
        sqrt_price_a: U256,
        sqrt_price_b: U256,
        liquidity: U256,
        round_up: bool,
    ) -> U256 {
        let (sqrt_price_start, sqrt_price_end) = if sqrt_price_a < sqrt_price_b {
            (sqrt_price_a, sqrt_price_b)
        } else {
            (sqrt_price_b, sqrt_price_a)
        };

        let numerator = liquidity * (sqrt_price_end - sqrt_price_start);

        if round_up {
            Self::div_rounding_up(numerator, Q96)
        } else {
            numerator / Q96
        }
    }

    /// Division with rounding up
    pub fn div_rounding_up(numerator: U256, denominator: U256) -> U256 {
        let quotient = numerator / denominator;
        let remainder = numerator % denominator;
        if remainder == U256_ZERO {
            quotient
        } else {
            quotient + U256_ONE
        }
    }

    /// Convert price to sqrt price X96 format
    pub fn price_to_sqrt_price_x96(price: f64) -> Result<U256, ProgramError> {
        let sqrt_price = (price as f64).sqrt();
        let sqrt_price_x96 = sqrt_price * 79228162514264337593543950336.0; // 2^96
        Ok(U256::from(sqrt_price_x96 as u128))
    }

    /// Convert sqrt price X96 to regular price
    pub fn sqrt_price_x96_to_price(sqrt_price_x96: U256) -> f64 {
        let sqrt_price = sqrt_price_x96.low_u128() as f64 / 79228162514264337593543950336.0; // 2^96
        sqrt_price * sqrt_price
    }

    /// Calculate liquidity for amounts
    pub fn get_liquidity_for_amounts(
        sqrt_price_a: U256,
        sqrt_price_b: U256,
        amount0: U256,
        amount1: U256,
    ) -> U256 {
        let (sqrt_price_lower, sqrt_price_upper) = if sqrt_price_a < sqrt_price_b {
            (sqrt_price_a, sqrt_price_b)
        } else {
            (sqrt_price_b, sqrt_price_a)
        };

        if sqrt_price_upper == sqrt_price_lower {
            return U256_ZERO;
        }

        let amount0_liquidity = amount0 * sqrt_price_lower * sqrt_price_upper / Q96;
        let amount1_liquidity = amount1 * Q96 / (sqrt_price_upper - sqrt_price_lower);

        if amount0_liquidity <= amount1_liquidity {
            amount0_liquidity
        } else {
            amount1_liquidity
        }
    }

    /// Calculate amounts for liquidity
    pub fn get_amounts_for_liquidity(
        sqrt_price_a: U256,
        sqrt_price_b: U256,
        liquidity: U256,
    ) -> (U256, U256) {
        let (sqrt_price_lower, sqrt_price_upper) = if sqrt_price_a < sqrt_price_b {
            (sqrt_price_a, sqrt_price_b)
        } else {
            (sqrt_price_b, sqrt_price_a)
        };

        let amount0 =
            Self::get_amount0_for_liquidity(sqrt_price_lower, sqrt_price_upper, liquidity);
        let amount1 =
            Self::get_amount1_for_liquidity(sqrt_price_lower, sqrt_price_upper, liquidity);

        (amount0, amount1)
    }
}

