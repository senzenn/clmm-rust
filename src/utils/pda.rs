use solana_program::{
    program_error::ProgramError,
    pubkey::Pubkey,
};

/// Pool PDA seeds
pub const POOL_SEED: &[u8] = b"pool";
pub const POOL_VAULT_SEED: &[u8] = b"pool_vault";
pub const POOL_AUTHORITY_SEED: &[u8] = b"pool_authority";

/// Position PDA seeds
pub const POSITION_SEED: &[u8] = b"position";

/// Tick PDA seeds
pub const TICK_SEED: &[u8] = b"tick";

/// Oracle PDA seeds
pub const ORACLE_SEED: &[u8] = b"oracle";

/// Derive the pool PDA address
pub fn derive_pool_address(
    program_id: &Pubkey,
    token_a: &Pubkey,
    token_b: &Pubkey,
    fee: u32,
) -> (Pubkey, u8) {
    let fee_bytes = fee.to_le_bytes();
    Pubkey::find_program_address(
        &[
            POOL_SEED,
            token_a.as_ref(),
            token_b.as_ref(),
            &fee_bytes,
        ],
        program_id,
    )
}

/// Derive the pool vault PDA address for token A
pub fn derive_pool_vault_a_address(
    program_id: &Pubkey,
    pool: &Pubkey,
) -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &[
            POOL_VAULT_SEED,
            pool.as_ref(),
            b"a",
        ],
        program_id,
    )
}

/// Derive the pool vault PDA address for token B
pub fn derive_pool_vault_b_address(
    program_id: &Pubkey,
    pool: &Pubkey,
) -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &[
            POOL_VAULT_SEED,
            pool.as_ref(),
            b"b",
        ],
        program_id,
    )
}

/// Derive the pool authority PDA address
pub fn derive_pool_authority_address(
    program_id: &Pubkey,
    pool: &Pubkey,
) -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &[
            POOL_AUTHORITY_SEED,
            pool.as_ref(),
        ],
        program_id,
    )
}

/// Derive the position PDA address
pub fn derive_position_address(
    program_id: &Pubkey,
    pool: &Pubkey,
    owner: &Pubkey,
    tick_lower: i32,
    tick_upper: i32,
) -> (Pubkey, u8) {
    let tick_lower_bytes = tick_lower.to_le_bytes();
    let tick_upper_bytes = tick_upper.to_le_bytes();

    Pubkey::find_program_address(
        &[
            POSITION_SEED,
            pool.as_ref(),
            owner.as_ref(),
            &tick_lower_bytes,
            &tick_upper_bytes,
        ],
        program_id,
    )
}

/// Derive the tick PDA address
pub fn derive_tick_address(
    program_id: &Pubkey,
    pool: &Pubkey,
    tick: i32,
) -> (Pubkey, u8) {
    let tick_bytes = tick.to_le_bytes();

    Pubkey::find_program_address(
        &[
            TICK_SEED,
            pool.as_ref(),
            &tick_bytes,
        ],
        program_id,
    )
}

/// Derive the oracle PDA address
pub fn derive_oracle_address(
    program_id: &Pubkey,
    pool: &Pubkey,
) -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &[
            ORACLE_SEED,
            pool.as_ref(),
        ],
        program_id,
    )
}

/// Verify that a derived address matches the expected PDA
pub fn verify_pda(
    expected: &Pubkey,
    seeds: &[&[u8]],
    program_id: &Pubkey,
) -> Result<u8, ProgramError> {
    let (derived, bump) = Pubkey::find_program_address(seeds, program_id);

    if &derived != expected {
        return Err(ProgramError::InvalidSeeds);
    }

    Ok(bump)
}

/// Create signer seeds for pool authority
pub fn pool_authority_seeds<'a>(
    pool: &'a Pubkey,
    bump: &'a [u8],
) -> [&'a [u8]; 3] {
    [
        POOL_AUTHORITY_SEED,
        pool.as_ref(),
        bump,
    ]
}

/// Create signer seeds for pool vault A
pub fn pool_vault_a_seeds<'a>(
    pool: &'a Pubkey,
    bump: &'a [u8],
) -> [&'a [u8]; 4] {
    [
        POOL_VAULT_SEED,
        pool.as_ref(),
        b"a",
        bump,
    ]
}

/// Create signer seeds for pool vault B
pub fn pool_vault_b_seeds<'a>(
    pool: &'a Pubkey,
    bump: &'a [u8],
) -> [&'a [u8]; 4] {
    [
        POOL_VAULT_SEED,
        pool.as_ref(),
        b"b",
        bump,
    ]
}
