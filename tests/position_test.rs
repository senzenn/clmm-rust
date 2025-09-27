use clmm_rust::state::{Position, PositionInfo};
use clmm_rust::math::tick_math::U256;
use clmm_rust::math::fixed_point::U256_ZERO;
use solana_program::pubkey::Pubkey;

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
