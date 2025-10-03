use solana_program::{
    account_info::{next_account_info, AccountInfo},
    entrypoint::ProgramResult,
    msg,
    program_error::ProgramError,
    pubkey::Pubkey,
};
use borsh::BorshDeserialize;
use crate::error::CLMMError;
use crate::state::{Pool, Position, Tick};
use crate::math::tick_math::{U256, I256, U256_ZERO, I256_ZERO};
use crate::utils::{
    assert_signer, assert_writable, assert_owned_by, assert_initialized,
    write_account_data, get_current_timestamp, token_transfer_signed,
    derive_tick_address, derive_pool_authority_address,
    pool_authority_seeds,
};

/// Remove liquidity from a position
///
/// Accounts expected:
/// 0. `[signer]` Position owner
/// 1. `[writable]` Pool account
/// 2. `[writable]` Position account
/// 3. `[writable]` Tick lower account
/// 4. `[writable]` Tick upper account
/// 5. `[writable]` User token A account
/// 6. `[writable]` User token B account
/// 7. `[writable]` Pool vault A
/// 8. `[writable]` Pool vault B
/// 9. `[]` Pool authority (PDA)
/// 10. `[]` Token program
pub fn process(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    liquidity_delta: u128,
    amount_0_min: u64,
    amount_1_min: u64,
) -> ProgramResult {
    msg!("Removing liquidity from position...");

    let account_info_iter = &mut accounts.iter();

    // Parse accounts
    let owner = next_account_info(account_info_iter)?;
    let pool_account = next_account_info(account_info_iter)?;
    let position_account = next_account_info(account_info_iter)?;
    let tick_lower_account = next_account_info(account_info_iter)?;
    let tick_upper_account = next_account_info(account_info_iter)?;
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
    assert_writable(tick_lower_account)?;
    assert_writable(tick_upper_account)?;
    assert_writable(user_token_a)?;
    assert_writable(user_token_b)?;
    assert_writable(vault_a)?;
    assert_writable(vault_b)?;

    // Validate pool is owned by this program
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

    // Validate liquidity delta
    if liquidity_delta == 0 {
        msg!("Liquidity delta cannot be zero");
        return Err(CLMMError::InsufficientLiquidity.into());
    }

    let liquidity_u256 = U256::from(liquidity_delta);

    // Validate position has enough liquidity
    if position.liquidity < liquidity_u256 {
        msg!("Insufficient position liquidity");
        return Err(CLMMError::InsufficientLiquidity.into());
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

    // Calculate amounts to withdraw
    let (amount_0, amount_1) = calculate_amounts_for_liquidity(
        &pool,
        position.tick_lower,
        position.tick_upper,
        liquidity_u256,
    )?;

    let amount_0_u64 = amount_0.low_u64();
    let amount_1_u64 = amount_1.low_u64();

    // Validate amounts meet minimums
    if amount_0_u64 < amount_0_min {
        msg!("Amount 0 ({}) below minimum ({})", amount_0_u64, amount_0_min);
        return Err(CLMMError::InsufficientLiquidity.into());
    }

    if amount_1_u64 < amount_1_min {
        msg!("Amount 1 ({}) below minimum ({})", amount_1_u64, amount_1_min);
        return Err(CLMMError::InsufficientLiquidity.into());
    }

    // Calculate fees earned
    let (fees_0, fees_1) = calculate_fees_earned(&pool, &position)?;
    let total_amount_0 = amount_0_u64.saturating_add(fees_0.low_u64());
    let total_amount_1 = amount_1_u64.saturating_add(fees_1.low_u64());

    // Update position liquidity
    position.liquidity = position.liquidity - liquidity_u256;
    position.updated_at = current_time;

    // Add fees to tokens owed
    position.add_tokens_owed(fees_0, fees_1);

    // Update ticks
    update_tick_liquidity(
        program_id,
        pool_account.key,
        tick_lower_account,
        position.tick_lower,
        I256::from_dec_str(&liquidity_delta.to_string()).unwrap_or(I256_ZERO),
        false, // lower tick - subtract liquidity
    )?;

    update_tick_liquidity(
        program_id,
        pool_account.key,
        tick_upper_account,
        position.tick_upper,
        I256::from_dec_str(&liquidity_delta.to_string()).unwrap_or(I256_ZERO),
        true, // upper tick - subtract liquidity
    )?;

    // Update pool liquidity if position is in range
    if pool.tick >= position.tick_lower && pool.tick < position.tick_upper {
        pool.liquidity = pool.liquidity - liquidity_u256;
        msg!("Updated pool liquidity: {}", pool.liquidity);
    }

    // Transfer tokens from pool vaults to user (principal + fees)
    let authority_bump_arr = [authority_bump];
    let authority_seeds = pool_authority_seeds(
        pool_account.key,
        &authority_bump_arr,
    );

    if total_amount_0 > 0 {
        msg!("Transferring {} of token A from pool to user (principal: {}, fees: {})",
            total_amount_0, amount_0_u64, fees_0.low_u64());
        token_transfer_signed(
            token_program,
            vault_a,
            user_token_a,
            pool_authority,
            total_amount_0,
            &authority_seeds,
        )?;
    }

    if total_amount_1 > 0 {
        msg!("Transferring {} of token B from pool to user (principal: {}, fees: {})",
            total_amount_1, amount_1_u64, fees_1.low_u64());
        token_transfer_signed(
            token_program,
            vault_b,
            user_token_b,
            pool_authority,
            total_amount_1,
            &authority_seeds,
        )?;
    }

    // Deactivate position if liquidity is zero
    if position.is_empty() {
        position.deactivate(current_time);
        msg!("Position deactivated (empty)");
    }

    // Save updated states
    write_account_data(position_account, &position)?;
    write_account_data(pool_account, &pool)?;

    msg!("Liquidity removed successfully");
    msg!("  Position: {}", position_account.key);
    msg!("  Liquidity removed: {}", liquidity_delta);
    msg!("  Amount 0 returned: {} (principal) + {} (fees)", amount_0_u64, fees_0.low_u64());
    msg!("  Amount 1 returned: {} (principal) + {} (fees)", amount_1_u64, fees_1.low_u64());
    msg!("  Remaining liquidity: {}", position.liquidity);

    Ok(())
}

/// Calculate token amounts for liquidity removal
fn calculate_amounts_for_liquidity(
    pool: &Pool,
    tick_lower: i32,
    tick_upper: i32,
    liquidity: U256,
) -> Result<(U256, U256), ProgramError> {
    use crate::math::TickMath;
    use crate::math::FixedPointMath;

    let sqrt_price_lower = TickMath::get_sqrt_ratio_at_tick(tick_lower)?;
    let sqrt_price_upper = TickMath::get_sqrt_ratio_at_tick(tick_upper)?;
    let current_sqrt_price = pool.sqrt_price_x96;

    let (amount_0, amount_1) = if current_sqrt_price <= sqrt_price_lower {
        // Price below range - only token0
        let amount_0 = FixedPointMath::get_amount0_delta(
            sqrt_price_lower,
            sqrt_price_upper,
            liquidity,
            false, // false for removal
        );
        (amount_0, U256_ZERO)
    } else if current_sqrt_price < sqrt_price_upper {
        // Price in range - both tokens
        let amount_0 = FixedPointMath::get_amount0_delta(
            current_sqrt_price,
            sqrt_price_upper,
            liquidity,
            false,
        );
        let amount_1 = FixedPointMath::get_amount1_delta(
            sqrt_price_lower,
            current_sqrt_price,
            liquidity,
            false,
        );
        (amount_0, amount_1)
    } else {
        // Price above range - only token1
        let amount_1 = FixedPointMath::get_amount1_delta(
            sqrt_price_lower,
            sqrt_price_upper,
            liquidity,
            false,
        );
        (U256_ZERO, amount_1)
    };

    Ok((amount_0, amount_1))
}

/// Calculate fees earned by a position
fn calculate_fees_earned(
    pool: &Pool,
    position: &Position,
) -> Result<(U256, U256), ProgramError> {
    // Calculate fee growth inside the position's range
    let fee_growth_inside_0 = pool.fee_growth_global0_x128;
    let fee_growth_inside_1 = pool.fee_growth_global1_x128;

    // Calculate fees earned since last update
    let fee_growth_delta_0 = fee_growth_inside_0 - position.fee_growth_inside0_last_x128;
    let fee_growth_delta_1 = fee_growth_inside_1 - position.fee_growth_inside1_last_x128;

    // Multiply by liquidity to get fee amounts
    let fees_0 = (position.liquidity * fee_growth_delta_0) / (U256::from(1u128) << 128);
    let fees_1 = (position.liquidity * fee_growth_delta_1) / (U256::from(1u128) << 128);

    Ok((fees_0, fees_1))
}

/// Update tick liquidity (for removal, liquidity_delta should be negative)
fn update_tick_liquidity(
    program_id: &Pubkey,
    pool_key: &Pubkey,
    tick_account: &AccountInfo,
    tick_index: i32,
    liquidity_delta: I256,
    upper: bool,
) -> ProgramResult {
    let (expected_tick, _tick_bump) = derive_tick_address(program_id, pool_key, tick_index);

    if tick_account.key != &expected_tick {
        msg!("Invalid tick PDA");
        return Err(ProgramError::InvalidSeeds);
    }

    assert_initialized(tick_account)?;

    // Load tick
    let tick_data = tick_account.try_borrow_data()?;
    let mut tick = Tick::deserialize(&mut &tick_data[..])?;
    drop(tick_data);

    // Update tick liquidity (negate delta for removal)
    let negative_delta = I256_ZERO - liquidity_delta;
    tick.update_liquidity(negative_delta, upper);

    // Save tick
    write_account_data(tick_account, &tick)?;

    Ok(())
}
