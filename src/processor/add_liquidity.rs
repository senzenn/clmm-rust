use solana_program::{
    account_info::{next_account_info, AccountInfo},
    entrypoint::ProgramResult,
    msg,
    program_error::ProgramError,
    pubkey::Pubkey,
    rent::Rent,
    sysvar::Sysvar,
};
use borsh::BorshDeserialize;
use crate::error::CLMMError;
use crate::state::{Pool, Position, Tick};
use crate::math::tick_math::{U256, I256, U256_ZERO, I256_ZERO};
use crate::utils::{
    assert_signer, assert_writable, assert_owned_by, assert_initialized,
    write_account_data, get_current_timestamp, token_transfer,
    create_account, derive_position_address, derive_tick_address,
    derive_pool_authority_address,
};

/// Add liquidity to a position
///
/// Accounts expected:
/// 0. `[signer]` Position owner
/// 1. `[writable]` Pool account
/// 2. `[writable]` Position account (PDA)
/// 3. `[writable]` Tick lower account (PDA)
/// 4. `[writable]` Tick upper account (PDA)
/// 5. `[writable]` User token A account
/// 6. `[writable]` User token B account
/// 7. `[writable]` Pool vault A
/// 8. `[writable]` Pool vault B
/// 9. `[]` Pool authority (PDA)
/// 10. `[]` Token program
/// 11. `[]` System program
/// 12. `[]` Rent sysvar
pub fn process(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    tick_lower: i32,
    tick_upper: i32,
    liquidity_delta: u128,
    amount_0_max: u64,
    amount_1_max: u64,
) -> ProgramResult {
    msg!("Adding liquidity to position...");

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
    let system_program = next_account_info(account_info_iter)?;
    let _rent_sysvar = next_account_info(account_info_iter)?;

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
    assert_initialized(pool_account)?;

    // Deserialize pool
    let pool_data = pool_account.try_borrow_data()?;
    let mut pool = Pool::deserialize(&mut &pool_data[..])?;
    drop(pool_data);

    // Validate liquidity delta
    if liquidity_delta == 0 {
        msg!("Liquidity delta cannot be zero");
        return Err(CLMMError::InsufficientLiquidity.into());
    }

    // Validate tick range
    pool.validate_tick_range(tick_lower, tick_upper)
        .map_err(|e| {
            msg!("Invalid tick range: {}", e);
            CLMMError::InvalidTickRange
        })?;

    // Get current timestamp
    let current_time = get_current_timestamp()? as u32;

    // Validate pool authority PDA
    let (expected_authority, _authority_bump) = derive_pool_authority_address(
        program_id,
        pool_account.key,
    );

    if pool_authority.key != &expected_authority {
        msg!("Invalid pool authority");
        return Err(ProgramError::InvalidSeeds);
    }

    // Calculate amounts needed
    let liquidity_u256 = U256::from(liquidity_delta);
    let (amount_0, amount_1) = calculate_amounts_for_liquidity(
        &pool,
        tick_lower,
        tick_upper,
        liquidity_u256,
    )?;

    // Validate amounts don't exceed maximums
    let amount_0_u64 = amount_0.low_u64();
    let amount_1_u64 = amount_1.low_u64();

    if amount_0_u64 > amount_0_max {
        msg!("Amount 0 ({}) exceeds maximum ({})", amount_0_u64, amount_0_max);
        return Err(CLMMError::InsufficientLiquidity.into());
    }

    if amount_1_u64 > amount_1_max {
        msg!("Amount 1 ({}) exceeds maximum ({})", amount_1_u64, amount_1_max);
        return Err(CLMMError::InsufficientLiquidity.into());
    }

    // Get rent
    let rent = Rent::get()?;

    // Handle position account (create or update)
    let (expected_position, position_bump) = derive_position_address(
        program_id,
        pool_account.key,
        owner.key,
        tick_lower,
        tick_upper,
    );

    if position_account.key != &expected_position {
        msg!("Invalid position PDA");
        return Err(ProgramError::InvalidSeeds);
    }

    let mut position = if position_account.data_is_empty() || position_account.lamports() == 0 {
        // Create new position
        msg!("Creating new position");

        let position_seeds = &[
            b"position",
            pool_account.key.as_ref(),
            owner.key.as_ref(),
            &tick_lower.to_le_bytes(),
            &tick_upper.to_le_bytes(),
            &[position_bump],
        ];

        let position_size = std::mem::size_of::<Position>() + 8;

        create_account(
            owner,
            position_account,
            system_program,
            program_id,
            &rent,
            position_size,
            position_seeds,
        )?;

        let position_id = pool.position_count;
        pool.position_count += 1;

        Position::new(
            *pool_account.key,
            *owner.key,
            tick_lower,
            tick_upper,
            position_id,
            current_time,
        ).map_err(|e| {
            msg!("Failed to create position: {}", e);
            CLMMError::InvalidTickRange
        })?
    } else {
        // Load existing position
        msg!("Updating existing position");
        let position_data = position_account.try_borrow_data()?;
        Position::deserialize(&mut &position_data[..])?
    };

    // Update position liquidity
    position.liquidity = position.liquidity + liquidity_u256;
    position.updated_at = current_time;

    // Handle ticks
    update_tick(
        program_id,
        pool_account.key,
        tick_lower_account,
        tick_lower,
        I256::from_dec_str(&liquidity_delta.to_string()).unwrap_or(I256_ZERO),
        false, // lower tick
        owner,
        system_program,
        &rent,
    )?;

    update_tick(
        program_id,
        pool_account.key,
        tick_upper_account,
        tick_upper,
        I256::from_dec_str(&liquidity_delta.to_string()).unwrap_or(I256_ZERO),
        true, // upper tick
        owner,
        system_program,
        &rent,
    )?;

    // Update pool liquidity if position is in range
    if pool.tick >= tick_lower && pool.tick < tick_upper {
        pool.liquidity = pool.liquidity + liquidity_u256;
        msg!("Updated pool liquidity: {}", pool.liquidity);
    }

    // Transfer tokens from user to pool vaults
    if amount_0_u64 > 0 {
        msg!("Transferring {} of token A from user to pool", amount_0_u64);
        token_transfer(
            token_program,
            user_token_a,
            vault_a,
            owner,
            amount_0_u64,
        )?;
    }

    if amount_1_u64 > 0 {
        msg!("Transferring {} of token B from user to pool", amount_1_u64);
        token_transfer(
            token_program,
            user_token_b,
            vault_b,
            owner,
            amount_1_u64,
        )?;
    }

    // Save updated states
    write_account_data(position_account, &position)?;
    write_account_data(pool_account, &pool)?;

    msg!("Liquidity added successfully");
    msg!("  Position: {}", position_account.key);
    msg!("  Liquidity: {}", liquidity_delta);
    msg!("  Amount 0: {}", amount_0_u64);
    msg!("  Amount 1: {}", amount_1_u64);
    msg!("  Tick range: [{}, {}]", tick_lower, tick_upper);

    Ok(())
}

/// Calculate token amounts needed for liquidity
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
        // Price below range - only token0 needed
        let amount_0 = FixedPointMath::get_amount0_delta(
            sqrt_price_lower,
            sqrt_price_upper,
            liquidity,
            true,
        );
        (amount_0, U256_ZERO)
    } else if current_sqrt_price < sqrt_price_upper {
        // Price in range - both tokens needed
        let amount_0 = FixedPointMath::get_amount0_delta(
            current_sqrt_price,
            sqrt_price_upper,
            liquidity,
            true,
        );
        let amount_1 = FixedPointMath::get_amount1_delta(
            sqrt_price_lower,
            current_sqrt_price,
            liquidity,
            true,
        );
        (amount_0, amount_1)
    } else {
        // Price above range - only token1 needed
        let amount_1 = FixedPointMath::get_amount1_delta(
            sqrt_price_lower,
            sqrt_price_upper,
            liquidity,
            true,
        );
        (U256_ZERO, amount_1)
    };

    Ok((amount_0, amount_1))
}

/// Update or create a tick
fn update_tick<'a>(
    program_id: &Pubkey,
    pool_key: &Pubkey,
    tick_account: &AccountInfo<'a>,
    tick_index: i32,
    liquidity_delta: I256,
    upper: bool,
    payer: &AccountInfo<'a>,
    system_program: &AccountInfo<'a>,
    rent: &Rent,
) -> ProgramResult {
    let (expected_tick, tick_bump) = derive_tick_address(program_id, pool_key, tick_index);

    if tick_account.key != &expected_tick {
        msg!("Invalid tick PDA");
        return Err(ProgramError::InvalidSeeds);
    }

    let mut tick = if tick_account.data_is_empty() || tick_account.lamports() == 0 {
        // Create new tick
        let tick_seeds = &[
            b"tick",
            pool_key.as_ref(),
            &tick_index.to_le_bytes(),
            &[tick_bump],
        ];

        let tick_size = std::mem::size_of::<Tick>() + 8;

        create_account(
            payer,
            tick_account,
            system_program,
            program_id,
            rent,
            tick_size,
            tick_seeds,
        )?;

        Tick::new(tick_index)
    } else {
        // Load existing tick
        let tick_data = tick_account.try_borrow_data()?;
        Tick::deserialize(&mut &tick_data[..])?
    };

    // Update tick liquidity
    tick.update_liquidity(liquidity_delta, upper);

    // Save tick
    write_account_data(tick_account, &tick)?;

    Ok(())
}
