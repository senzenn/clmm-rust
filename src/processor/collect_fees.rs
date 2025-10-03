use solana_program::{
    account_info::{next_account_info, AccountInfo},
    entrypoint::ProgramResult,
    msg,
    program_error::ProgramError,
    pubkey::Pubkey,
};
use borsh::BorshDeserialize;
use crate::error::CLMMError;
use crate::state::{Pool, Position};
use crate::math::tick_math::{U256, U256_ZERO};
use crate::utils::{
    assert_signer, assert_writable, assert_owned_by, assert_initialized,
    write_account_data, get_current_timestamp, token_transfer_signed,
    derive_pool_authority_address, pool_authority_seeds,
};

/// Collect fees from a position
///
/// Accounts expected:
/// 0. `[signer]` Position owner
/// 1. `[writable]` Pool account
/// 2. `[writable]` Position account
/// 3. `[writable]` User token A account (recipient)
/// 4. `[writable]` User token B account (recipient)
/// 5. `[writable]` Pool vault A
/// 6. `[writable]` Pool vault B
/// 7. `[]` Pool authority (PDA)
/// 8. `[]` Token program
pub fn process(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    amount_0_requested: u64,
    amount_1_requested: u64,
) -> ProgramResult {
    msg!("Collecting fees from position...");

    let account_info_iter = &mut accounts.iter();

    // Parse accounts
    let owner = next_account_info(account_info_iter)?;
    let pool_account = next_account_info(account_info_iter)?;
    let position_account = next_account_info(account_info_iter)?;
    let user_token_a = next_account_info(account_info_iter)?;
    let user_token_b = next_account_info(account_info_iter)?;
    let vault_a = next_account_info(account_info_iter)?;
    let vault_b = next_account_info(account_info_iter)?;
    let pool_authority = next_account_info(account_info_iter)?;
    let token_program = next_account_info(account_info_iter)?;

    // Validate owner is signer
    assert_signer(owner)?;

    // Validate writable accounts
    assert_writable(pool_account)?;
    assert_writable(position_account)?;
    assert_writable(user_token_a)?;
    assert_writable(user_token_b)?;
    assert_writable(vault_a)?;
    assert_writable(vault_b)?;

    // Validate accounts are owned by this program
    assert_owned_by(pool_account, program_id)?;
    assert_owned_by(position_account, program_id)?;
    assert_initialized(pool_account)?;
    assert_initialized(position_account)?;

    // Deserialize pool
    let pool_data = pool_account.try_borrow_data()?;
    let mut pool = Pool::deserialize(&mut &pool_data[..])?;
    drop(pool_data);

    // Deserialize position
    let position_data = position_account.try_borrow_data()?;
    let mut position = Position::deserialize(&mut &position_data[..])?;
    drop(position_data);

    // Validate position owner
    if &position.owner != owner.key {
        msg!("Position owner mismatch");
        return Err(CLMMError::Unauthorized.into());
    }

    // Validate position is active
    if !position.is_active {
        msg!("Position is not active");
        return Err(CLMMError::InvalidAccount.into());
    }

    // Get current timestamp
    let current_time = get_current_timestamp()? as u32;

    // Validate pool authority PDA
    let (expected_authority, authority_bump) = derive_pool_authority_address(
        program_id,
        pool_account.key,
    );

    if pool_authority.key != &expected_authority {
        msg!("Invalid pool authority");
        return Err(ProgramError::InvalidSeeds);
    }

    // Calculate all fees earned (including already owed)
    let (accrued_fees_0, accrued_fees_1) = calculate_accrued_fees(&pool, &mut position)?;

    // Add newly accrued fees to tokens owed
    position.add_tokens_owed(accrued_fees_0, accrued_fees_1);

    // Update fee growth tracking to current values
    position.update_fee_growth(
        pool.fee_growth_global0_x128,
        pool.fee_growth_global1_x128,
        current_time,
    );

    // Determine amounts to collect
    let amount_0_to_collect = if amount_0_requested == 0 || amount_0_requested > position.tokens_owed0.low_u64() {
        position.tokens_owed0.low_u64()
    } else {
        amount_0_requested
    };

    let amount_1_to_collect = if amount_1_requested == 0 || amount_1_requested > position.tokens_owed1.low_u64() {
        position.tokens_owed1.low_u64()
    } else {
        amount_1_requested
    };

    // Check if there are fees to collect
    if amount_0_to_collect == 0 && amount_1_to_collect == 0 {
        msg!("No fees to collect");
        return Ok(());
    }

    // Collect tokens from position
    let (collected_0, collected_1) = position.collect_tokens_owed(
        U256::from(amount_0_to_collect),
        U256::from(amount_1_to_collect),
    );

    let collected_0_u64 = collected_0.low_u64();
    let collected_1_u64 = collected_1.low_u64();

    // Update pool protocol fees (if any)
    // Note: In a production system, a percentage of fees might go to the protocol
    // For now, all fees go to liquidity providers

    // Transfer collected fees from pool vaults to user
    let authority_bump_arr = [authority_bump];
    let authority_seeds = pool_authority_seeds(
        pool_account.key,
        &authority_bump_arr,
    );

    if collected_0_u64 > 0 {
        msg!("Transferring {} of token A fees to user", collected_0_u64);
        token_transfer_signed(
            token_program,
            vault_a,
            user_token_a,
            pool_authority,
            collected_0_u64,
            &authority_seeds,
        )?;

        // Update pool protocol fees tracking
        pool.protocol_fees_token0 = pool.protocol_fees_token0 + collected_0;
    }

    if collected_1_u64 > 0 {
        msg!("Transferring {} of token B fees to user", collected_1_u64);
        token_transfer_signed(
            token_program,
            vault_b,
            user_token_b,
            pool_authority,
            collected_1_u64,
            &authority_seeds,
        )?;

        // Update pool protocol fees tracking
        pool.protocol_fees_token1 = pool.protocol_fees_token1 + collected_1;
    }

    // Update position timestamp
    position.updated_at = current_time;

    // Save updated states
    write_account_data(position_account, &position)?;
    write_account_data(pool_account, &pool)?;

    msg!("Fees collected successfully");
    msg!("  Position: {}", position_account.key);
    msg!("  Token A fees collected: {}", collected_0_u64);
    msg!("  Token B fees collected: {}", collected_1_u64);
    msg!("  Token A fees remaining: {}", position.tokens_owed0);
    msg!("  Token B fees remaining: {}", position.tokens_owed1);

    Ok(())
}

/// Calculate fees accrued since last update
fn calculate_accrued_fees(
    pool: &Pool,
    position: &Position,
) -> Result<(U256, U256), ProgramError> {
    // If position has no liquidity, no new fees accrued
    if position.liquidity == U256_ZERO {
        return Ok((U256_ZERO, U256_ZERO));
    }

    // Calculate fee growth inside the position's range since last update
    let fee_growth_delta_0 = pool.fee_growth_global0_x128
        .checked_sub(position.fee_growth_inside0_last_x128)
        .unwrap_or(U256_ZERO);

    let fee_growth_delta_1 = pool.fee_growth_global1_x128
        .checked_sub(position.fee_growth_inside1_last_x128)
        .unwrap_or(U256_ZERO);

    // Calculate fees: (liquidity * fee_growth_delta) / 2^128
    let fees_0 = (position.liquidity * fee_growth_delta_0) / (U256::from(1u128) << 128);
    let fees_1 = (position.liquidity * fee_growth_delta_1) / (U256::from(1u128) << 128);

    msg!("Accrued fees since last update: {} token A, {} token B", fees_0, fees_1);

    Ok((fees_0, fees_1))
}

/// Calculate fee growth inside a tick range
/// This is a simplified version - in production, you'd need to fetch tick data
#[allow(dead_code)]
fn calculate_fee_growth_inside(
    pool: &Pool,
    tick_lower: i32,
    tick_upper: i32,
    fee_growth_outside_lower_0: U256,
    fee_growth_outside_lower_1: U256,
    fee_growth_outside_upper_0: U256,
    fee_growth_outside_upper_1: U256,
) -> (U256, U256) {
    let current_tick = pool.tick;

    // Calculate fee growth below lower tick
    let fee_growth_below_0;
    let fee_growth_below_1;

    if current_tick >= tick_lower {
        fee_growth_below_0 = fee_growth_outside_lower_0;
        fee_growth_below_1 = fee_growth_outside_lower_1;
    } else {
        fee_growth_below_0 = pool.fee_growth_global0_x128 - fee_growth_outside_lower_0;
        fee_growth_below_1 = pool.fee_growth_global1_x128 - fee_growth_outside_lower_1;
    }

    // Calculate fee growth above upper tick
    let fee_growth_above_0;
    let fee_growth_above_1;

    if current_tick < tick_upper {
        fee_growth_above_0 = fee_growth_outside_upper_0;
        fee_growth_above_1 = fee_growth_outside_upper_1;
    } else {
        fee_growth_above_0 = pool.fee_growth_global0_x128 - fee_growth_outside_upper_0;
        fee_growth_above_1 = pool.fee_growth_global1_x128 - fee_growth_outside_upper_1;
    }

    // Calculate fee growth inside the range
    let fee_growth_inside_0 = pool.fee_growth_global0_x128
        - fee_growth_below_0
        - fee_growth_above_0;

    let fee_growth_inside_1 = pool.fee_growth_global1_x128
        - fee_growth_below_1
        - fee_growth_above_1;

    (fee_growth_inside_0, fee_growth_inside_1)
}
