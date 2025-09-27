use clmm_rust::math::tick_math::{TickMath, MIN_TICK, MAX_TICK, U256_ZERO};

#[test]
fn test_get_sqrt_ratio_at_tick() {
    let ratio = TickMath::get_sqrt_ratio_at_tick(0).unwrap();
    assert!(ratio > U256_ZERO);

    let ratio_min = TickMath::get_sqrt_ratio_at_tick(MIN_TICK).unwrap();
    let ratio_max = TickMath::get_sqrt_ratio_at_tick(MAX_TICK).unwrap();
    assert!(ratio_max > ratio_min);
}

#[test]
fn test_tick_math_bounds() {
    assert!(TickMath::get_sqrt_ratio_at_tick(MIN_TICK - 1).is_err());
    assert!(TickMath::get_sqrt_ratio_at_tick(MAX_TICK + 1).is_err());
}
