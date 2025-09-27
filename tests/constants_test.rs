use clmm_rust::state::constants::*;

#[test]
fn test_fee_tiers() {
    assert_eq!(FEE_TIER_0_01, 1);
    assert_eq!(FEE_TIER_0_05, 5);
    assert_eq!(FEE_TIER_0_3, 30);
    assert_eq!(FEE_TIER_1_0, 100);
}

#[test]
fn test_tick_spacings() {
    assert_eq!(TICK_SPACING_1, 1);
    assert_eq!(TICK_SPACING_10, 10);
    assert_eq!(TICK_SPACING_60, 60);
    assert_eq!(TICK_SPACING_200, 200);
}

#[test]
fn test_account_sizes() {
    assert!(POOL_ACCOUNT_SIZE > 0);
    assert!(POSITION_ACCOUNT_SIZE > 0);
    assert!(TICK_ACCOUNT_SIZE > 0);
}
