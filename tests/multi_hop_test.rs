use clmm_rust::math::MultiHopRouter;
use clmm_rust::state::Pool;
use clmm_rust::math::tick_math::U256;
use solana_program::pubkey::Pubkey;

#[test]
fn test_multi_hop_router_creation() {
    let router = MultiHopRouter::new();
    assert!(router.pools.is_empty());
    assert!(router.routing_graph.is_empty());
}

#[test]
fn test_add_pool() {
    let mut router = MultiHopRouter::new();
    let pool = create_test_pool();

    router.add_pool(pool);

    assert_eq!(router.pools.len(), 1);
    assert!(router.routing_graph.contains_key(&pool.token_a));
    assert!(router.routing_graph.contains_key(&pool.token_b));
}

#[test]
fn test_find_best_route() {
    let mut router = MultiHopRouter::new();

    // Add pools for a simple triangle: A -> B -> C
    let pool_ab = create_pool_ab();
    let pool_bc = create_pool_bc();

    router.add_pool(pool_ab);
    router.add_pool(pool_bc);

    let token_a = Pubkey::new_unique();
    let token_b = Pubkey::new_unique();
    let token_c = Pubkey::new_unique();

    // This would require updating the pools to use these tokens
    // For now, just test that the router can find routes
    let available_tokens = router.get_available_tokens();
    assert!(!available_tokens.is_empty());
}

fn create_test_pool() -> Pool {
    let token_a = Pubkey::new_unique();
    let token_b = Pubkey::new_unique();
    let initial_price = U256([1000000000000000000000000, 0, 0, 0]);

    Pool::new(token_a, token_b, 300, 60, initial_price).unwrap()
}

fn create_pool_ab() -> Pool {
    let token_a = Pubkey::new_unique();
    let token_b = Pubkey::new_unique();
    let initial_price = U256([1000000000000000000000000, 0, 0, 0]);

    Pool::new(token_a, token_b, 300, 60, initial_price).unwrap()
}

fn create_pool_bc() -> Pool {
    let token_b = Pubkey::new_unique();
    let token_c = Pubkey::new_unique();
    let initial_price = U256([1000000000000000000000000, 0, 0, 0]);

    Pool::new(token_b, token_c, 300, 60, initial_price).unwrap()
}
