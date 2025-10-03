use crate::error::CLMMError;
use crate::math::tick_math::{U256, U256_ZERO};
use crate::math::dynamic_fee::MarketDataPoint;
use crate::state::Pool;
use solana_program::{program_error::ProgramError, pubkey::Pubkey};
use std::collections::{HashMap, VecDeque};

/// Multi-hop swap routing system for complex swap paths
pub struct MultiHopRouter {
    /// Available pools indexed by (token_a, token_b, fee)
    pub pools: HashMap<(Pubkey, Pubkey, u32), Pool>,
    /// Routing graph for path finding
    pub routing_graph: HashMap<Pubkey, Vec<(Pubkey, u32)>>,
}

impl MultiHopRouter {
    /// Create a new multi-hop router
    pub fn new() -> Self {
        Self {
            pools: HashMap::new(),
            routing_graph: HashMap::new(),
        }
    }

    /// Add a pool to the router
    pub fn add_pool(&mut self, pool: Pool) {
        let key = (pool.token_a, pool.token_b, pool.fee);

        // Add to pools map
        self.pools.insert(key, pool.clone());

        // Update routing graph
        self.routing_graph
            .entry(pool.token_a)
            .or_insert_with(Vec::new)
            .push((pool.token_b, pool.fee));

        self.routing_graph
            .entry(pool.token_b)
            .or_insert_with(Vec::new)
            .push((pool.token_a, pool.fee));
    }

    /// Find the best multi-hop route
    pub fn find_best_route(
        &self,
        token_in: &Pubkey,
        token_out: &Pubkey,
        amount_in: U256,
        max_hops: u8,
    ) -> Result<MultiHopRoute, ProgramError> {
        let mut best_route = None;
        let mut best_output = U256_ZERO;

        // Try all possible routes up to max_hops
        for hops in 1..=max_hops {
            let routes = self.find_routes(token_in, token_out, hops);
            for route in routes {
                let output = self.estimate_route_output(&route, amount_in)?;
                if output > best_output {
                    best_output = output;
                    best_route = Some(route);
                }
            }
        }

        best_route.ok_or(CLMMError::InvalidInstruction.into())
    }

    /// Find all possible routes with a given number of hops
    fn find_routes(&self, token_in: &Pubkey, token_out: &Pubkey, hops: u8) -> Vec<MultiHopRoute> {
        let mut routes = Vec::new();
        let mut current_paths = vec![vec![*token_in]];

        for _ in 0..hops {
            let mut next_paths = Vec::new();
            for path in current_paths {
                let last_token = *path.last().unwrap();

                if let Some(neighbors) = self.routing_graph.get(&last_token) {
                    for (neighbor, fee) in neighbors {
                        if !path.contains(neighbor) {
                            let mut new_path = path.clone();
                            new_path.push(*neighbor);
                            next_paths.push((new_path, *fee));
                        }
                    }
                }
            }
            current_paths = next_paths.into_iter().map(|(path, _)| path).collect();
        }

        // Filter routes that end with token_out
        for path in current_paths {
            if let Some(last) = path.last() {
                if last == token_out && path.len() > 1 {
                    let mut route = MultiHopRoute {
                        path: path.clone(),
                        fees: Vec::new(),
                        pools: Vec::new(),
                    };

                    // Extract fees for each hop
                    for i in 0..path.len() - 1 {
                        let token_a = path[i];
                        let token_b = path[i + 1];

                        // Find the fee for this hop
                        if let Some(neighbors) = self.routing_graph.get(&token_a) {
                            for (neighbor, fee) in neighbors {
                                if neighbor == &token_b {
                                    route.fees.push(*fee);
                                    break;
                                }
                            }
                        }
                    }

                    routes.push(route);
                }
            }
        }

        routes
    }

    /// Estimate output for a multi-hop route
    fn estimate_route_output(&self, route: &MultiHopRoute, amount_in: U256) -> Result<U256, ProgramError> {
        let mut current_amount = amount_in;

        for i in 0..route.path.len() - 1 {
            let token_in = route.path[i];
            let token_out = route.path[i + 1];
            let fee = route.fees[i];

            // Find the pool for this hop
            let pool_key = if token_in < token_out {
                (token_in, token_out, fee)
            } else {
                (token_out, token_in, fee)
            };

            if let Some(pool) = self.pools.get(&pool_key) {
                let zero_for_one = token_in < token_out;
                current_amount = Self::estimate_single_hop_output(pool, current_amount, zero_for_one)?;
            } else {
                return Ok(U256_ZERO); // Pool not found
            }
        }

        Ok(current_amount)
    }

    /// Estimate output for a single hop
    fn estimate_single_hop_output(
        pool: &Pool,
        amount_in: U256,
        zero_for_one: bool,
    ) -> Result<U256, ProgramError> {
        if pool.liquidity == U256_ZERO {
            return Ok(U256_ZERO);
        }

        // Calculate fee
        let fee_amount = amount_in * U256::from(pool.fee) / U256::from(10000);
        let amount_after_fee = amount_in - fee_amount;

        if zero_for_one {
            // Token0 -> Token1
            let price_ratio = pool.sqrt_price_x96 * pool.sqrt_price_x96 / crate::math::tick_math::Q96;
            Ok(amount_after_fee * crate::math::tick_math::Q96 / price_ratio)
        } else {
            // Token1 -> Token0
            let price_ratio = crate::math::tick_math::Q96 * crate::math::tick_math::Q96 / (pool.sqrt_price_x96 * pool.sqrt_price_x96);
            Ok(amount_after_fee * price_ratio / crate::math::tick_math::Q96)
        }
    }

    /// Execute a multi-hop swap
    pub fn execute_multi_hop_swap(
        &mut self,
        route: &MultiHopRoute,
        amount_in: U256,
        minimum_amount_out: U256,
        recipient: &Pubkey,
    ) -> Result<MultiHopSwapResult, ProgramError> {
        let mut current_amount = amount_in;
        let mut total_fees_paid = U256_ZERO;
        let mut pools_used = Vec::new();

        for i in 0..route.path.len() - 1 {
            let token_in = route.path[i];
            let token_out = route.path[i + 1];
            let fee = route.fees[i];

            // Find the pool for this hop
            let pool_key = if token_in < token_out {
                (token_in, token_out, fee)
            } else {
                (token_out, token_in, fee)
            };

            if let Some(pool) = self.pools.get_mut(&pool_key) {
                let zero_for_one = token_in < token_out;

                // Execute single hop swap
                let mut price_history = VecDeque::new();
                let mut volume_history = VecDeque::new();
                let mut impact_history = VecDeque::new();
                let mut oracle_observations = VecDeque::new();
                let hop_result = crate::math::SwapEngine::execute_swap(
                    pool,
                    current_amount,
                    zero_for_one,
                    crate::math::tick_math::U256::MAX, // No price limit for intermediate hops
                    recipient,
                    &mut price_history,
                    &mut volume_history,
                    &mut impact_history,
                    &mut oracle_observations,
                    1000, // Use a fixed timestamp for now
                    1, // Sequence number
                )?;

                current_amount = hop_result.amount_out;
                total_fees_paid += hop_result.amount_in - hop_result.amount_out;
                pools_used.push(pool_key);
            } else {
                return Err(CLMMError::InvalidInstruction.into());
            }
        }

        if current_amount < minimum_amount_out {
            return Err(CLMMError::InsufficientLiquidity.into());
        }

        Ok(MultiHopSwapResult {
            amount_in,
            amount_out: current_amount,
            total_fees_paid,
            hops: route.path.len() - 1,
            pools_used,
            path: route.path.clone(),
        })
    }

    /// Get available tokens in the system
    pub fn get_available_tokens(&self) -> Vec<Pubkey> {
        let mut tokens = std::collections::HashSet::new();
        for (token_a, token_b, _) in self.pools.keys() {
            tokens.insert(*token_a);
            tokens.insert(*token_b);
        }
        tokens.into_iter().collect()
    }

    /// Get pools for a specific token pair
    pub fn get_pools_for_pair(&self, token_a: &Pubkey, token_b: &Pubkey) -> Vec<&Pool> {
        let mut pools = Vec::new();

        let key1 = (*token_a, *token_b, 0);
        let key2 = (*token_b, *token_a, 0);

        if let Some(pool) = self.pools.get(&key1) {
            pools.push(pool);
        }
        if let Some(pool) = self.pools.get(&key2) {
            pools.push(pool);
        }

        pools
    }
}

/// Multi-hop route representation
#[derive(Debug, Clone)]
pub struct MultiHopRoute {
    pub path: Vec<Pubkey>,      // Token path: [token_in, intermediate1, ..., token_out]
    pub fees: Vec<u32>,         // Fee for each hop
    pub pools: Vec<(Pubkey, Pubkey, u32)>, // Pool keys for each hop
}

/// Result of a multi-hop swap execution
#[derive(Debug, Clone)]
pub struct MultiHopSwapResult {
    pub amount_in: U256,
    pub amount_out: U256,
    pub total_fees_paid: U256,
    pub hops: usize,
    pub pools_used: Vec<(Pubkey, Pubkey, u32)>,
    pub path: Vec<Pubkey>,
}

impl MultiHopSwapResult {
    /// Calculate the effective exchange rate
    pub fn effective_rate(&self) -> f64 {
        self.amount_out.low_u128() as f64 / self.amount_in.low_u128() as f64
    }

    /// Calculate total fees as percentage of input
    pub fn fee_percentage(&self) -> f64 {
        (self.total_fees_paid.low_u128() as f64 / self.amount_in.low_u128() as f64) * 100.0
    }
}

