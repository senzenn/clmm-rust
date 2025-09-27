use clmm_rust::math::price_impact::{PriceImpactCalculator, ImpactSeverity, U256_ZERO};
use clmm_rust::state::Pool;
use clmm_rust::math::tick_math::U256;
use solana_program::pubkey::Pubkey;

#[test]
fn test_price_impact_calculation() {
    let pool = create_test_pool();
    let amount_in = U256::from(1000u64);
    let result = PriceImpactCalculator::calculate_price_impact(&pool, amount_in, true).unwrap();

    assert!(result.impact_bps >= 0 && result.impact_bps <= 10000);
    assert!(result.expected_price >= 0.0);
}

#[test]
fn test_optimal_swap_amount() {
    let pool = create_test_pool();
    let target_impact = 100; // 1%
    let optimal_amount = PriceImpactCalculator::calculate_optimal_swap_amount(&pool, target_impact, true).unwrap();

    // Should get some reasonable amount
    assert!(optimal_amount > U256_ZERO);
}

#[test]
fn test_impact_severity_classification() {
    assert_eq!(
        PriceImpactCalculator::classify_impact_severity(25),
        ImpactSeverity::Negligible
    );
    assert_eq!(
        PriceImpactCalculator::classify_impact_severity(150),
        ImpactSeverity::Low
    );
    assert_eq!(
        PriceImpactCalculator::classify_impact_severity(350),
        ImpactSeverity::Medium
    );
    assert_eq!(
        PriceImpactCalculator::classify_impact_severity(1000),
        ImpactSeverity::High
    );
    assert_eq!(
        PriceImpactCalculator::classify_impact_severity(5000),
        ImpactSeverity::Critical
    );
}

fn create_test_pool() -> Pool {
    let token_a = Pubkey::new_unique();
    let token_b = Pubkey::new_unique();
    let initial_price = U256([1000000000000000000000000, 0, 0, 0]); // 1e21

    Pool::new(token_a, token_b, 300, 60, initial_price).unwrap()
}
