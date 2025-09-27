use crate::error::CLMMError;
use crate::math::tick_math::{U256, I256, Q96};
use solana_program::program_error::ProgramError;

pub struct FixedPointMath;

impl FixedPointMath {
    /// Multiply two U256 numbers and divide by a denominator with rounding up
    pub fn mul_div_rounding_up(
        x: U256,
        y: U256,
        denominator: U256,
    ) -> Result<U256, ProgramError> {
        let result = Self::mul_div(x, y, denominator)?;
        if x * y % denominator != U256::zero() {
            Ok(result + U256::one())
        } else {
            Ok(result)
        }
    }

    /// Multiply two U256 numbers and divide by a denominator
    pub fn mul_div(x: U256, y: U256, denominator: U256) -> Result<U256, ProgramError> {
        if denominator == U256::zero() {
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

    /// Calculate square root of a U256 number
    pub fn sqrt(x: U256) -> Result<U256, ProgramError> {
        if x == U256::zero() {
            return Ok(U256::zero());
        }

        let mut z = x;
        let mut y = (x + U256::one()) >> 1;

        while y < z {
            z = y;
            y = (x / y + y) >> 1;
        }

        Ok(z)
    }

    /// Get amount0 for given liquidity and price range
    pub fn get_amount0_for_liquidity(
        sqrt_a: U256,
        sqrt_b: U256,
        liquidity: U256,
    ) -> U256 {
        if sqrt_a > sqrt_b {
            Self::get_amount0_for_liquidity(sqrt_b, sqrt_a, liquidity)
        } else {
            Self::mul_div(sqrt_a, sqrt_b, Q96) * liquidity / Q96
        }
    }

    /// Get amount1 for given liquidity and price range
    pub fn get_amount1_for_liquidity(
        sqrt_a: U256,
        sqrt_b: U256,
        liquidity: U256,
    ) -> U256 {
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

        let amount0 = Self::div_rounding_up(numerator1 * numerator2, sqrt_price_end * sqrt_price_start);

        if round_up && (numerator1 * numerator2 % (sqrt_price_end * sqrt_price_start) != U256::zero()) {
            amount0 + U256::one()
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
        if remainder == U256::zero() {
            quotient
        } else {
            quotient + U256::one()
        }
    }

    /// Convert price to sqrt price X96 format
    pub fn price_to_sqrt_price_x96(price: f64) -> Result<U256, ProgramError> {
        let sqrt_price = (price as f64).sqrt();
        let sqrt_price_x96 = sqrt_price * (1u64 << 96) as f64;
        Ok(U256::from(sqrt_price_x96 as u128))
    }

    /// Convert sqrt price X96 to regular price
    pub fn sqrt_price_x96_to_price(sqrt_price_x96: U256) -> f64 {
        let sqrt_price = sqrt_price_x96.low_u128() as f64 / (1u64 << 96) as f64;
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
            return U256::zero();
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

        let amount0 = Self::get_amount0_for_liquidity(sqrt_price_lower, sqrt_price_upper, liquidity);
        let amount1 = Self::get_amount1_for_liquidity(sqrt_price_lower, sqrt_price_upper, liquidity);

        (amount0, amount1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mul_div() {
        let x = U256::from(100u64);
        let y = U256::from(200u64);
        let denominator = U256::from(1000u64);

        let result = FixedPointMath::mul_div(x, y, denominator).unwrap();
        assert_eq!(result, U256::from(20u64));
    }

    #[test]
    fn test_sqrt() {
        let x = U256::from(4u64);
        let sqrt_x = FixedPointMath::sqrt(x).unwrap();
        assert_eq!(sqrt_x, U256::from(2u64));

        let x = U256::from(9u64);
        let sqrt_x = FixedPointMath::sqrt(x).unwrap();
        assert_eq!(sqrt_x, U256::from(3u64));
    }

    #[test]
    fn test_price_conversion() {
        let price = 100.0;
        let sqrt_price_x96 = FixedPointMath::price_to_sqrt_price_x96(price).unwrap();
        let converted_price = FixedPointMath::sqrt_price_x96_to_price(sqrt_price_x96);

        let diff = (converted_price - price).abs();
        assert!(diff < 0.01);
    }

    #[test]
    fn test_get_liquidity_for_amounts() {
        let sqrt_price_a = U256::from(1000000000000000000000000u128); // 1e21
        let sqrt_price_b = U256::from(2000000000000000000000000u128); // 2e21
        let amount0 = U256::from(1000u64);
        let amount1 = U256::from(2000u64);

        let liquidity = FixedPointMath::get_liquidity_for_amounts(
            sqrt_price_a,
            sqrt_price_b,
            amount0,
            amount1,
        );

        assert!(liquidity > U256::zero());
    }
}
