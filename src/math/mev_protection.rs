use crate::error::CLMMError;
use crate::math::tick_math::{U256, U256_ZERO};
use crate::state::Pool;
use solana_program::program_error::ProgramError;
use std::collections::{VecDeque, HashMap};

///  TWAP calculation
#[derive(Debug, Clone)]
pub struct OracleObservation {
    pub timestamp: u32,
    pub price: U256,
    pub tick: i32,
    pub liquidity: U256,
}

//auction
#[derive(Debug, Clone)]
pub struct BatchAuctionEntry {
    pub user: solana_program::pubkey::Pubkey,
    pub amount_in: U256,
    pub min_amount_out: U256,
    pub zero_for_one: bool,
    pub timestamp: u32,
    pub sequence_number: u64,
}

#[derive(Debug, Clone)]
pub enum BatchOperation {
    Swap {
        user: solana_program::pubkey::Pubkey,
        amount_in: U256,
        min_amount_out: U256,
        zero_for_one: bool,
        sqrt_price_limit: U256,
    },
    AddLiquidity {
        user: solana_program::pubkey::Pubkey,
        pool_id: solana_program::pubkey::Pubkey,
        tick_lower: i32,
        tick_upper: i32,
        amount_0: U256,
        amount_1: U256,
    },
    RemoveLiquidity {
        user: solana_program::pubkey::Pubkey,
        pool_id: solana_program::pubkey::Pubkey,
        position_id: solana_program::pubkey::Pubkey,
        liquidity_amount: U256,
    },
}

#[derive(Debug, Clone)]
pub struct BatchState {
    pub operations: VecDeque<BatchOperation>,
    pub total_operations: usize,
    pub batch_start_time: u32,
    pub last_execution_time: u32,
    pub gas_budget: u64,
    pub gas_used: u64,
    pub successful_operations: usize,
    pub failed_operations: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MevConfig {
    pub oracle_window: u32,          // TWAP window in seconds
    pub min_update_interval: u32,    // Minimum time between position updates
    pub max_slippage_bps: u32,       // Maximum allowed slippage (basis points)
    pub batch_auction_enabled: bool, // Whether batch auctions are enabled
    pub batch_window: u32,           // Batch auction window in seconds
    pub oracle_enabled: bool,        // Whether oracle price validation is enabled
}

/// Configuration for Twitter/social media monitoring
#[derive(Debug, Clone, PartialEq)]
pub struct SocialMediaConfig {
    pub twitter_enabled: bool,
    pub sentiment_threshold: i32,    // Sentiment score threshold (-100 to 100)
    pub volume_threshold: u32,       // Minimum tweet volume to trigger alert
    pub influencer_threshold: u32,   // Minimum followers for influencer status
    pub monitoring_window: u32,      // Time window for social media analysis (seconds)
    pub keywords: Vec<String>,       // Keywords to monitor
}

/// Social media post data
#[derive(Debug, Clone)]
pub struct SocialMediaData {
    pub timestamp: u32,
    pub platform: String,            // "twitter", "discord", etc.
    pub author: String,
    pub author_followers: u64,
    pub content: String,
    pub sentiment_score: i32,        // -100 (very negative) to 100 (very positive)
    pub retweets: u32,
    pub likes: u32,
    pub mentions: Vec<String>,
    pub hashtags: Vec<String>,
    pub urls: Vec<String>,
}

/// Aggregated social media metrics
#[derive(Debug, Clone)]
pub struct SocialMediaMetrics {
    pub total_volume: u32,
    pub average_sentiment: f64,
    pub positive_ratio: f64,
    pub negative_ratio: f64,
    pub influencer_activity: u32,
    pub spam_score: f64,
    pub manipulation_probability: f64,
}

/// Implements multiple layers of protection against Miner Extractable Value attacks
pub struct MevProtectionEngine;

impl MevProtectionEngine {
    /// Default MEV protection configuration
    pub fn default_config() -> MevConfig {
        MevConfig {
            oracle_window: 300,      // 5 minutes
            min_update_interval: 60, // 1 minute
            max_slippage_bps: 1000,  // 10%
            batch_auction_enabled: true,
            batch_window: 30, // 30 seconds
            oracle_enabled: true,
        }
    }

    /// Initialize social media monitoring configuration
    pub fn social_media_config() -> SocialMediaConfig {
        SocialMediaConfig {
            twitter_enabled: true,
            sentiment_threshold: 20,     // Alert if sentiment > 20 or < -20
            volume_threshold: 100,       // Alert if > 100 tweets about token
            influencer_threshold: 10000, // Accounts with > 10k followers
            monitoring_window: 3600,     // 1 hour window
            keywords: vec![
                "pump".to_string(),
                "moon".to_string(),
                "rug".to_string(),
                "scam".to_string(),
                "buy".to_string(),
                "sell".to_string(),
            ],
        }
    }

    pub fn validate_twap_vs_spot(
        oracle_observations: &VecDeque<OracleObservation>,
        spot_price: U256,
        config: &MevConfig,
    ) -> Result<bool, ProgramError> {
        if !config.oracle_enabled || oracle_observations.len() < 2 {
            return Ok(true); // Skip validation if disabled or insufficient data
        }

        let twap = Self::calculate_twap(oracle_observations, config.oracle_window)?;

        // Calculate price deviation
        let price_diff = if twap > spot_price {
            twap - spot_price
        } else {
            spot_price - twap
        };

        let deviation_bps = (price_diff * U256::from(10000)) / twap;

        // Reject if deviation exceeds maximum allowed slippage
        Ok(deviation_bps <= U256::from(config.max_slippage_bps))
    }

    pub fn calculate_twap(
        observations: &VecDeque<OracleObservation>,
        window: u32,
    ) -> Result<U256, ProgramError> {
        if observations.len() < 2 {
            return Err(CLMMError::InvalidOracle.into());
        }

        let current_time = observations.back().unwrap().timestamp;
        let window_start = current_time.saturating_sub(window);

        let mut valid_observations: Vec<_> = observations
            .iter()
            .filter(|obs| obs.timestamp >= window_start)
            .collect();

        if valid_observations.is_empty() {
            return Err(CLMMError::InvalidOracle.into());
        }

        valid_observations.sort_by_key(|obs| obs.timestamp);

        let mut total_weighted_sum = U256_ZERO;
        let mut total_time_weight = U256_ZERO;

        // Use the first observation
        let mut prev_time = valid_observations[0].timestamp;
        let mut prev_price = valid_observations[0].price;

        // Calculate TWAP using linear interpolation between observations
        for obs in &valid_observations[1..] {
            let current_time_point = obs.timestamp;
            let current_price = obs.price;

            // Only consider intervals within the window
            let interval_start = prev_time.max(window_start);
            let interval_end = current_time_point.min(current_time);

            if interval_end > interval_start {
                let interval_duration = U256::from(interval_end - interval_start);
                let avg_price = (prev_price + current_price) / U256::from(2);

                total_weighted_sum = total_weighted_sum + (avg_price * interval_duration);
                total_time_weight = total_time_weight + interval_duration;
            }

            prev_time = current_time_point;
            prev_price = current_price;
        }

        if total_time_weight == U256_ZERO {
            return Err(CLMMError::InvalidOracle.into());
        }

        Ok(total_weighted_sum / total_time_weight)
    }

    pub fn validate_update_frequency(
        last_update: u32,
        current_time: u32,
        config: &MevConfig,
    ) -> Result<bool, ProgramError> {
        let time_since_update = current_time.saturating_sub(last_update);
        Ok(time_since_update >= config.min_update_interval)
    }

    pub fn process_batch_auction(
        pending_swaps: &mut VecDeque<BatchAuctionEntry>,
        current_time: u32,
        config: &MevConfig,
    ) -> Result<Vec<BatchAuctionEntry>, ProgramError> {
        if !config.batch_auction_enabled {
            return Ok(Vec::new());
        }

        let mut executed_swaps = Vec::new();

        while let Some(entry) = pending_swaps.front() {
            if current_time.saturating_sub(entry.timestamp) >= config.batch_window {
                if let Some(entry) = pending_swaps.pop_front() {
                    executed_swaps.push(entry);
                }
            } else {
                break;
            }
        }

        Ok(executed_swaps)
    }

    pub fn process_enhanced_batch(
        batch_state: &mut BatchState,
        current_time: u32,
        config: &MevConfig,
    ) -> Result<Vec<BatchOperation>, ProgramError> {
        let mut executed_operations = Vec::new();

        if current_time.saturating_sub(batch_state.batch_start_time) < config.batch_window {
            return Ok(executed_operations);
        }

        if batch_state.gas_used >= batch_state.gas_budget {
            return Ok(executed_operations);
        }

        while let Some(_operation) = batch_state.operations.front() {
            // Check gas budget before each operation
            if batch_state.gas_used >= batch_state.gas_budget {
                break;
            }

            if let Some(executed_op) = batch_state.operations.pop_front() {
                executed_operations.push(executed_op);
                batch_state.successful_operations += 1;
            }
        }

        batch_state.last_execution_time = current_time;
        Ok(executed_operations)
    }

    pub fn add_to_batch(
        batch_state: &mut BatchState,
        operation: BatchOperation,
        current_time: u32,
    ) -> Result<(), ProgramError> {
        // Initialize batch if this is the first operation
        if batch_state.operations.is_empty() {
            batch_state.batch_start_time = current_time;
            batch_state.last_execution_time = current_time;
        }

        batch_state.operations.push_back(operation);
        batch_state.total_operations += 1;

        Ok(())
    }

    pub fn get_batch_stats(batch_state: &BatchState) -> BatchStatistics {
        let elapsed_time = batch_state
            .last_execution_time
            .saturating_sub(batch_state.batch_start_time);
        let success_rate = if batch_state.total_operations > 0 {
            (batch_state.successful_operations * 100) / batch_state.total_operations
        } else {
            0
        };

        BatchStatistics {
            total_operations: batch_state.total_operations,
            successful_operations: batch_state.successful_operations,
            failed_operations: batch_state.failed_operations,
            elapsed_time,
            success_rate,
            gas_used: batch_state.gas_used,
            gas_budget: batch_state.gas_budget,
        }
    }

    pub fn create_batch_state(gas_budget: u64) -> BatchState {
        BatchState {
            operations: VecDeque::new(),
            total_operations: 0,
            batch_start_time: 0,
            last_execution_time: 0,
            gas_budget,
            gas_used: 0,
            successful_operations: 0,
            failed_operations: 0,
        }
    }

    pub fn validate_transaction_ordering(
        sequence_number: u64,
        last_processed_sequence: u64,
    ) -> Result<bool, ProgramError> {
        // Ensure sequence numbers are processed in order
        Ok(sequence_number == last_processed_sequence + 1)
    }

    /// Calculate MEV-resistant fee based on market conditions and TWAP deviation
    pub fn calculate_mev_resistant_fee(
        spot_price: U256,
        twap_price: U256,
        base_fee: u32,
        _config: &MevConfig,
    ) -> Result<u32, ProgramError> {
        let price_diff = if spot_price > twap_price {
            spot_price - twap_price
        } else {
            twap_price - spot_price
        };

        let deviation_bps = (price_diff * U256::from(10000)) / twap_price;

        // Increase fee based on price deviation from TWAP
        let mut adjusted_fee = base_fee;

        if deviation_bps > U256::from(500) {
            // > 5% deviation
            adjusted_fee = adjusted_fee.saturating_mul(3); // Triple the fee
        } else if deviation_bps > U256::from(200) {
            // > 2% deviation
            adjusted_fee = adjusted_fee.saturating_mul(2); // Double the fee
        } else if deviation_bps > U256::from(100) {
            // > 1% deviation
            adjusted_fee = (adjusted_fee * 3) / 2; // 1.5x the fee
        }

        Ok(adjusted_fee.min(1000).max(1)) // Between 0.01% and 10%
    }

    pub fn update_oracle_observations(
        observations: &mut VecDeque<OracleObservation>,
        pool: &Pool,
        current_time: u32,
        max_observations: usize,
    ) -> Result<(), ProgramError> {
        let observation = OracleObservation {
            timestamp: current_time,
            price: pool.sqrt_price_x96,
            tick: pool.tick,
            liquidity: pool.liquidity,
        };

        observations.push_back(observation);

        while observations.len() > max_observations {
            observations.pop_front();
        }

        Ok(())
    }

    pub fn validate_swap_mev_protection(
        pool: &Pool,
        _amount_in: U256,
        zero_for_one: bool,
        sqrt_price_limit: U256,
        oracle_observations: &VecDeque<OracleObservation>,
        config: &MevConfig,
    ) -> Result<bool, ProgramError> {
        // 1. Validate TWAP vs spot price
        if !Self::validate_twap_vs_spot(oracle_observations, pool.sqrt_price_x96, config)? {
            return Ok(false);
        }

        // 2. Check price limit against TWAP
        let twap = Self::calculate_twap(oracle_observations, config.oracle_window)?;

        if zero_for_one {
            // Price decreasing - limit should be >= TWAP
            if sqrt_price_limit < twap {
                return Ok(false);
            }
        } else {
            // Price increasing - limit should be <= TWAP
            if sqrt_price_limit > twap {
                return Ok(false);
            }
        }

        Ok(true)
    }

    pub fn get_mev_protection_status(
        pool: &Pool,
        oracle_observations: &VecDeque<OracleObservation>,
        config: &MevConfig,
    ) -> Result<MevProtectionStatus, ProgramError> {
        let twap = Self::calculate_twap(oracle_observations, config.oracle_window)?;
        let spot_price = pool.sqrt_price_x96;

        let price_diff = if spot_price > twap {
            spot_price - twap
        } else {
            twap - spot_price
        };

        let deviation_bps = (price_diff * U256::from(10000)) / twap;

        Ok(MevProtectionStatus {
            twap_price: twap,
            spot_price,
            deviation_bps: deviation_bps.low_u32(),
            oracle_observations_count: oracle_observations.len(),
            protection_enabled: config.oracle_enabled,
        })
    }

    /// Analyze social media sentiment for MEV protection
    pub fn analyze_social_media_sentiment(
        social_data: &VecDeque<SocialMediaData>,
        config: &SocialMediaConfig,
        current_time: u32,
    ) -> Result<SocialMediaMetrics, ProgramError> {
        if social_data.is_empty() {
            return Ok(SocialMediaMetrics {
                total_volume: 0,
                average_sentiment: 0.0,
                positive_ratio: 0.0,
                negative_ratio: 0.0,
                influencer_activity: 0,
                spam_score: 0.0,
                manipulation_probability: 0.0,
            });
        }

        // Filter data within monitoring window
        let window_start = current_time.saturating_sub(config.monitoring_window);
        let recent_data: Vec<_> = social_data
            .iter()
            .filter(|data| data.timestamp >= window_start)
            .collect();

        if recent_data.is_empty() {
            return Ok(SocialMediaMetrics {
                total_volume: 0,
                average_sentiment: 0.0,
                positive_ratio: 0.0,
                negative_ratio: 0.0,
                influencer_activity: 0,
                spam_score: 0.0,
                manipulation_probability: 0.0,
            });
        }

        let total_volume = recent_data.len() as u32;
        let total_sentiment: i32 = recent_data.iter().map(|data| data.sentiment_score).sum();
        let average_sentiment = total_sentiment as f64 / total_volume as f64;

        let positive_count = recent_data.iter().filter(|data| data.sentiment_score > 10).count();
        let negative_count = recent_data.iter().filter(|data| data.sentiment_score < -10).count();
        let positive_ratio = positive_count as f64 / total_volume as f64;
        let negative_ratio = negative_count as f64 / total_volume as f64;

        // Calculate influencer activity (accounts with > threshold followers)
        let influencer_activity = recent_data
            .iter()
            .filter(|data| data.author_followers >= config.influencer_threshold as u64)
            .count() as u32;

        // Calculate spam score based on repetitive content and bot-like behavior
        let spam_score = Self::calculate_spam_score(&recent_data);

        // Calculate manipulation probability based on unusual patterns
        let manipulation_probability = Self::calculate_manipulation_probability(
            &recent_data,
            average_sentiment,
            positive_ratio,
            influencer_activity,
            spam_score,
        );

        Ok(SocialMediaMetrics {
            total_volume,
            average_sentiment,
            positive_ratio,
            negative_ratio,
            influencer_activity,
            spam_score,
            manipulation_probability,
        })
    }

    /// Calculate spam score based on content patterns
    fn calculate_spam_score(data: &[&SocialMediaData]) -> f64 {
        let mut spam_indicators = 0u32;
        let total_posts = data.len() as f64;

        if total_posts < 2.0 {
            return 0.0;
        }

        // Check for repetitive content
        let mut content_similarity = 0u32;
        for i in 0..data.len() {
            for j in (i + 1)..data.len() {
                if Self::calculate_text_similarity(&data[i].content, &data[j].content) > 0.8 {
                    content_similarity += 1;
                }
            }
        }
        if content_similarity as f64 / total_posts > 0.3 {
            spam_indicators += 1;
        }

        // Check for excessive caps and emojis
        let mut caps_excessive = 0u32;
        for post in data {
            let caps_ratio = post.content.chars().filter(|c| c.is_uppercase()).count() as f64
                / post.content.len().max(1) as f64;
            if caps_ratio > 0.7 {
                caps_excessive += 1;
            }
        }
        if caps_excessive as f64 / total_posts > 0.4 {
            spam_indicators += 1;
        }

        (spam_indicators as f64 / 2.0).min(1.0)
    }

    /// Calculate text similarity using simple Jaccard index
    fn calculate_text_similarity(text1: &str, text2: &str) -> f64 {
        let text1_lower = text1.to_lowercase();
        let text2_lower = text2.to_lowercase();
        let words1: HashMap<&str, bool> = text1_lower.split_whitespace().map(|w| (w, true)).collect();
        let words2: HashMap<&str, bool> = text2_lower.split_whitespace().map(|w| (w, true)).collect();

        let intersection = words1.keys().filter(|k| words2.contains_key(*k)).count();
        let union = words1.len() + words2.len() - intersection;

        if union == 0 {
            0.0
        } else {
            intersection as f64 / union as f64
        }
    }

    /// Calculate probability of social media manipulation
    fn calculate_manipulation_probability(
        data: &[&SocialMediaData],
        avg_sentiment: f64,
        positive_ratio: f64,
        influencer_activity: u32,
        spam_score: f64,
    ) -> f64 {
        let mut probability = 0.0;

        // High sentiment deviation from normal
        if avg_sentiment > 30.0 || avg_sentiment < -30.0 {
            probability += 0.3;
        }

        // Unusual positive/negative ratio
        if positive_ratio > 0.8 || positive_ratio < 0.2 {
            probability += 0.25;
        }

        // High influencer activity in short time
        if influencer_activity > (data.len() as u32 / 4) {
            probability += 0.2;
        }

        // High spam score
        if spam_score > 0.6 {
            probability += 0.25;
        }

        // Sudden volume spike
        if data.len() > 50 {
            probability += 0.1;
        }

        if probability > 1.0 {
            1.0
        } else {
            probability
        }
    }

    /// Enhanced MEV validation including social media sentiment
    pub fn validate_enhanced_mev_protection(
        pool: &Pool,
        _amount_in: U256,
        zero_for_one: bool,
        sqrt_price_limit: U256,
        oracle_observations: &VecDeque<OracleObservation>,
        social_data: &VecDeque<SocialMediaData>,
        config: &MevConfig,
        social_config: &SocialMediaConfig,
        current_time: u32,
    ) -> Result<bool, ProgramError> {
        // 1. Standard TWAP validation
        if !Self::validate_twap_vs_spot(oracle_observations, pool.sqrt_price_x96, config)? {
            return Ok(false);
        }

        // 2. Social media sentiment analysis
        if social_config.twitter_enabled && !social_data.is_empty() {
            let metrics = Self::analyze_social_media_sentiment(social_data, social_config, current_time)?;

            // Block transactions during high manipulation probability
            if metrics.manipulation_probability > 0.7 {
                return Ok(false);
            }

            // Enhanced validation during unusual social media activity
            if metrics.total_volume > social_config.volume_threshold {
                // Require stricter price limits during social media hype
                let twap = Self::calculate_twap(oracle_observations, config.oracle_window)?;
                if zero_for_one {
                    // Price decreasing - check if limit is too aggressive
                    if sqrt_price_limit < twap * U256::from(95) / U256::from(100) { // 5% stricter
                        return Ok(false);
                    }
                } else {
                    // Price increasing - check if limit is too aggressive
                    if sqrt_price_limit > twap * U256::from(105) / U256::from(100) { // 5% stricter
                        return Ok(false);
                    }
                }
            }
        }

        // 3. Influencer activity check
        if social_config.twitter_enabled && !social_data.is_empty() {
            let metrics = Self::analyze_social_media_sentiment(social_data, social_config, current_time)?;

            if metrics.influencer_activity > 10 && metrics.average_sentiment > 40.0 {
                // High influencer activity with positive sentiment - potential pump & dump
                let twap = Self::calculate_twap(oracle_observations, config.oracle_window)?;
                let spot_price = pool.sqrt_price_x96;
                let deviation = ((spot_price.max(twap) - spot_price.min(twap)) * U256::from(10000)) / twap;

                // Require tighter deviation limits during influencer hype
                if deviation > U256::from(500) { // 5% instead of normal 10%
                    return Ok(false);
                }
            }
        }

        Ok(true)
    }

    /// Add social media data point
    pub fn add_social_media_data(
        social_data: &mut VecDeque<SocialMediaData>,
        data: SocialMediaData,
        max_entries: usize,
    ) -> Result<(), ProgramError> {
        social_data.push_back(data);

        // Maintain maximum entries to prevent memory bloat
        while social_data.len() > max_entries {
            social_data.pop_front();
        }

        Ok(())
    }

    /// Generate social media-based MEV protection report
    pub fn generate_social_mev_report(
        pool: &Pool,
        oracle_observations: &VecDeque<OracleObservation>,
        social_data: &VecDeque<SocialMediaData>,
        config: &MevConfig,
        social_config: &SocialMediaConfig,
        current_time: u32,
    ) -> Result<SocialMevReport, ProgramError> {
        let twap = Self::calculate_twap(oracle_observations, config.oracle_window)?;
        let spot_price = pool.sqrt_price_x96;

        let price_diff = if spot_price > twap {
            spot_price - twap
        } else {
            twap - spot_price
        };

        let deviation_bps = (price_diff * U256::from(10000)) / twap;

        let social_metrics = if social_config.twitter_enabled && !social_data.is_empty() {
            Some(Self::analyze_social_media_sentiment(social_data, social_config, current_time)?)
        } else {
            None
        };

        Ok(SocialMevReport {
            timestamp: current_time,
            twap_price: twap,
            spot_price,
            price_deviation_bps: deviation_bps.low_u32(),
            oracle_observations_count: oracle_observations.len(),
            social_media_metrics: social_metrics,
            protection_enabled: config.oracle_enabled,
            social_protection_enabled: social_config.twitter_enabled,
        })
    }
}

#[derive(Debug, Clone)]
pub struct MevProtectionStatus {
    pub twap_price: U256,
    pub spot_price: U256,
    pub deviation_bps: u32,
    pub oracle_observations_count: usize,
    pub protection_enabled: bool,
}

#[derive(Debug, Clone)]
pub struct SocialMevReport {
    pub timestamp: u32,
    pub twap_price: U256,
    pub spot_price: U256,
    pub price_deviation_bps: u32,
    pub oracle_observations_count: usize,
    pub social_media_metrics: Option<SocialMediaMetrics>,
    pub protection_enabled: bool,
    pub social_protection_enabled: bool,
}

#[derive(Debug, Clone)]
pub struct BatchStatistics {
    pub total_operations: usize,
    pub successful_operations: usize,
    pub failed_operations: usize,
    pub elapsed_time: u32,
    pub success_rate: usize,
    pub gas_used: u64,
    pub gas_budget: u64,
}

impl borsh::BorshSerialize for MevConfig {
    fn serialize<W: std::io::Write>(&self, writer: &mut W) -> std::io::Result<()> {
        self.oracle_window.serialize(writer)?;
        self.min_update_interval.serialize(writer)?;
        self.max_slippage_bps.serialize(writer)?;
        self.batch_auction_enabled.serialize(writer)?;
        self.batch_window.serialize(writer)?;
        self.oracle_enabled.serialize(writer)?;
        Ok(())
    }
}

impl borsh::BorshDeserialize for MevConfig {
    fn deserialize(buf: &mut &[u8]) -> std::io::Result<Self> {
        let oracle_window = u32::deserialize(buf)?;
        let min_update_interval = u32::deserialize(buf)?;
        let max_slippage_bps = u32::deserialize(buf)?;
        let batch_auction_enabled = bool::deserialize(buf)?;
        let batch_window = u32::deserialize(buf)?;
        let oracle_enabled = bool::deserialize(buf)?;

        Ok(MevConfig {
            oracle_window,
            min_update_interval,
            max_slippage_bps,
            batch_auction_enabled,
            batch_window,
            oracle_enabled,
        })
    }

    fn deserialize_reader<R: std::io::Read>(reader: &mut R) -> std::io::Result<Self> {
        let oracle_window = u32::deserialize_reader(reader)?;
        let min_update_interval = u32::deserialize_reader(reader)?;
        let max_slippage_bps = u32::deserialize_reader(reader)?;
        let batch_auction_enabled = bool::deserialize_reader(reader)?;
        let batch_window = u32::deserialize_reader(reader)?;
        let oracle_enabled = bool::deserialize_reader(reader)?;

        Ok(MevConfig {
            oracle_window,
            min_update_interval,
            max_slippage_bps,
            batch_auction_enabled,
            batch_window,
            oracle_enabled,
        })
    }
}
