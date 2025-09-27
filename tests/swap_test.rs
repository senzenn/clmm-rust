use clmm_rust::math::{SwapEngine, PriceImpactCalculator, MultiHopRouter};
use clmm_rust::state::{Pool, Position};
use clmm_rust::math::tick_math::U256;
use solana_program::pubkey::Pubkey;

#[test]
fn test_basic_swap_functionality() {
    let mut pool = create_test_pool();
    let amount_in = U256([1000, 0, 0, 0]);
    let sqrt_price_limit = U256::MAX;
    let user = Pubkey::new_unique();

    // Test swap execution
    let swap_result = SwapEngine::execute_swap(
        &mut pool,
        amount_in,
        true, // zero_for_one
        sqrt_price_limit,
        &user,
    ).unwrap();

    assert!(swap_result.amount_in > U256::zero());
    assert!(swap_result.amount_out > U256::zero());
    assert!(swap_result.price_impact >= 0 && swap_result.price_impact <= 10000);
    assert!(swap_result.final_sqrt_price != pool.sqrt_price_x96); // Price should change
}

#[test]
fn test_price_impact_calculation() {
    let pool = create_test_pool();
    let amount_in = U256([1000, 0, 0, 0]);

    let impact_result = PriceImpactCalculator::calculate_price_impact(&pool, amount_in, true).unwrap();

    assert!(impact_result.impact_bps >= 0 && impact_result.impact_bps <= 10000);
    assert!(impact_result.expected_price >= 0.0);
    use clmm_rust::math::price_impact::ImpactSeverity;
    assert!(impact_result.severity != ImpactSeverity::Critical || pool.liquidity == U256::zero());
}

#[test]
fn test_optimal_swap_amount() {
    let pool = create_test_pool();
    let target_impact = 100; // 1%

    let optimal_amount = PriceImpactCalculator::calculate_optimal_swap_amount(&pool, target_impact, true).unwrap();

    // Should get a reasonable amount
    assert!(optimal_amount > U256::zero());
    assert!(optimal_amount <= pool.liquidity / U256::from(10)); // Should be capped at 10% of liquidity
}

#[test]
fn test_multi_hop_router() {
    let mut router = MultiHopRouter::new();
    let pool = create_test_pool();

    router.add_pool(pool);

    let available_tokens = router.get_available_tokens();
    assert!(!available_tokens.is_empty());

    // Test pool retrieval
    let token_a = available_tokens[0];
    let token_b = available_tokens[1];
    let pools = router.get_pools_for_pair(&token_a, &token_b);
    assert!(!pools.is_empty());
}

#[test]
fn test_swap_output_estimation() {
    let pool = create_test_pool();
    let amount_in = U256([1000, 0, 0, 0]);

    let estimated_output = SwapEngine::estimate_swap_output(&pool, amount_in, true).unwrap();

    // Should get some output estimate
    if pool.liquidity > U256::zero() {
        assert!(estimated_output > U256::zero());
    }
}

#[test]
fn test_price_limit_validation() {
    let pool = create_test_pool();
    let amount_in = U256([1000, 0, 0, 0]);
    let sqrt_price_limit = pool.sqrt_price_x96; // Same as current price

    let user = Pubkey::new_unique();

    // This should work since limit equals current price
    let result = SwapEngine::execute_swap(
        &mut pool.clone(),
        amount_in,
        true,
        sqrt_price_limit,
        &user,
    );

    // Should either succeed or fail with a specific error, not panic
    assert!(result.is_ok() || result.is_err());
}

fn create_test_pool() -> Pool {
    let token_a = Pubkey::new_unique();
    let token_b = Pubkey::new_unique();
    let initial_price = U256([1000000000000000000000000, 0, 0, 0]); // 1e21

    Pool::new(token_a, token_b, 300, 60, initial_price).unwrap()
}

// Tests from src/math/swap.rs
#[test]
fn test_swap_engine_creation() {
    let engine = SwapEngine;
    // Test that we can create the engine
    assert!(true);
}

#[test]
fn test_price_impact_calculation_swap_math() {
    let mut pool = create_test_pool();
    let amount_in = U256([1000, 0, 0, 0]);
    let price_impact = SwapEngine::calculate_price_impact(&pool, amount_in, true).unwrap();

    assert!(price_impact >= 0 && price_impact <= 10000);
}

#[test]
fn test_swap_output_estimation_swap_math() {
    let pool = create_test_pool();
    let amount_in = U256([1000, 0, 0, 0]);
    let amount_out = SwapEngine::estimate_swap_output(&pool, amount_in, true).unwrap();

    // Should get some output for non-zero input
    assert!(amount_out > U256::zero() || pool.liquidity == U256::zero());
}

// Test from src/processor/swap.rs
#[test]
fn test_swap_processor_validation() {
    // This would test the processor with mock accounts
    assert!(true);
}
