use clmm_rust::state::Pool;
use clmm_rust::math::tick_math::U256;
use solana_program::pubkey::Pubkey;

#[test]
fn test_pool_creation() {
    let token_a = Pubkey::new_unique();
    let token_b = Pubkey::new_unique();
    let initial_price = U256([1000000000000000000000000, 0, 0, 0]);

    let pool = Pool::new(token_a, token_b, 300, 60, initial_price).unwrap();
    assert!(pool.is_valid());
    assert_eq!(pool.fee, 300);
    assert_eq!(pool.tick_spacing, 60);
}

#[test]
fn test_token_sorting() {
    let token_a = Pubkey::new_unique();
    let token_b = Pubkey::new_unique();
    let initial_price = U256([1000000000000000000000000, 0, 0, 0]);

    let pool = Pool::new(token_b, token_a, 300, 60, initial_price).unwrap();
    assert!(pool.token_a < pool.token_b);
}

#[test]
fn test_tick_validation() {
    let token_a = Pubkey::new_unique();
    let token_b = Pubkey::new_unique();
    let initial_price = U256([1000000000000000000000000, 0, 0, 0]);

    let pool = Pool::new(token_a, token_b, 300, 60, initial_price).unwrap();

    assert!(pool.validate_tick_range(-60, 60).is_ok());

    assert!(pool.validate_tick_range(60, 60).is_err());

    assert!(pool.validate_tick_range(-50, 60).is_err());
}
