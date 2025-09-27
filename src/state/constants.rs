use solana_program::pubkey::Pubkey;

/// Constants used throughout the CLMM program
pub const MINIMUM_LIQUIDITY: u64 = 1000;
pub const MAX_FEE: u32 = 10000;
pub const PROTOCOL_FEE_PERCENT: u32 = 0;

/// Common fee tiers (in basis points)
pub const FEE_TIER_0_01: u32 = 1;
pub const FEE_TIER_0_05: u32 = 5;
pub const FEE_TIER_0_3: u32 = 30;
pub const FEE_TIER_1_0: u32 = 100;

/// Common tick spacings
pub const TICK_SPACING_1: u32 = 1;
pub const TICK_SPACING_10: u32 = 10;
pub const TICK_SPACING_60: u32 = 60;
pub const TICK_SPACING_200: u32 = 200;

pub const MAX_POSITIONS_PER_POOL: u64 = 100000;

pub const MAX_TICK_RANGE_WIDTH: u32 = 887272 * 2;

/// PDA seeds
pub const POOL_SEED: &[u8] = b"pool";
pub const POSITION_SEED: &[u8] = b"position";
pub const TICK_SEED: &[u8] = b"tick";
pub const BITMAP_SEED: &[u8] = b"bitmap";
pub const PROTOCOL_FEE_SEED: &[u8] = b"protocol_fee";

/// Account sizes (in bytes)
pub const POOL_ACCOUNT_SIZE: usize = 8 + 32 + 32 + 4 + 4 + 4 + 16 + 4 + 16 + 16 + 16 + 16 + 16 + 8 + 4 + 1 + 256;

pub const POSITION_ACCOUNT_SIZE: usize = 8 + 32 + 32 + 4 + 4 + 16 + 16 + 16 + 16 + 16 + 8 + 4 + 4 + 1 + 256;

pub const TICK_ACCOUNT_SIZE: usize = 8 + 4 + 16 + 16 + 16 + 16 + 16 + 16 + 4 + 1 + 256;

/// Helper function to get pool PDA
pub fn get_pool_pda(token_a: &Pubkey, token_b: &Pubkey, fee: u32, program_id: &Pubkey) -> (Pubkey, u8) {
    let seeds = &[
        POOL_SEED,
        &token_a.to_bytes(),
        &token_b.to_bytes(),
        &fee.to_le_bytes(),
        &[0],
    ];
    Pubkey::find_program_address(seeds, program_id)
}

/// Helper function to get position PDA
pub fn get_position_pda(pool_id: &Pubkey, owner: &Pubkey, tick_lower: i32, tick_upper: i32, program_id: &Pubkey) -> (Pubkey, u8) {
    let seeds = &[
        POSITION_SEED,
        &pool_id.to_bytes(),
        &owner.to_bytes(),
        &tick_lower.to_le_bytes(),
        &tick_upper.to_le_bytes(),
        &[0],
    ];
    Pubkey::find_program_address(seeds, program_id)
}

/// Helper function to get tick PDA
pub fn get_tick_pda(pool_id: &Pubkey, tick: i32, program_id: &Pubkey) -> (Pubkey, u8) {
    let seeds = &[
        TICK_SEED,
        &pool_id.to_bytes(),
        &tick.to_le_bytes(),
        &[0],
    ];
    Pubkey::find_program_address(seeds, program_id)
}

/// Helper function to get bitmap PDA
pub fn get_bitmap_pda(pool_id: &Pubkey, program_id: &Pubkey) -> (Pubkey, u8) {
    let seeds = &[
        BITMAP_SEED,
        &pool_id.to_bytes(),
        &[0],
    ];
    Pubkey::find_program_address(seeds, program_id)
}

