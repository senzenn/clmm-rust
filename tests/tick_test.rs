use clmm_rust::state::{Tick, TickBitmap, TickInfo};
use clmm_rust::math::tick_math::{U256, I256};

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
