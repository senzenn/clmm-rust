use clmm_rust::math::fixed_point::{FixedPointMath, U256_ZERO};
use clmm_rust::math::tick_math::U256;

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

    let liquidity =
        FixedPointMath::get_liquidity_for_amounts(sqrt_price_a, sqrt_price_b, amount0, amount1);

    assert!(liquidity > U256_ZERO);
}
