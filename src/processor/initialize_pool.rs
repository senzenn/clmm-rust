use solana_program::{
    account_info::{next_account_info, AccountInfo},
    entrypoint::ProgramResult,
    msg,
    program_error::ProgramError,
    pubkey::Pubkey,
    rent::Rent,
    sysvar::Sysvar,
};
use crate::error::CLMMError;
use crate::state::Pool;
use crate::utils::{
    create_account, assert_signer,
    write_account_data, token_initialize_account,
    derive_pool_address, derive_pool_vault_a_address, derive_pool_vault_b_address,
    derive_pool_authority_address,
};
use crate::math::tick_math::U256;

// System program ID
solana_program::declare_id!("Fw4mNHEDrHAGg41XEcp7DkHpEP12MiUcCrP2Lj5ngth9");

/// Initialize a new concentrated liquidity pool
///
/// Accounts expected:
/// 0. `[signer]` Payer account
/// 1. `[writable]` Pool account (PDA)
/// 2. `[]` Token A mint
/// 3. `[]` Token B mint
/// 4. `[writable]` Pool vault for token A (PDA)
/// 5. `[writable]` Pool vault for token B (PDA)
/// 6. `[]` Pool authority (PDA)
/// 7. `[]` Token program
/// 8. `[]` System program
/// 9. `[]` Rent sysvar
pub fn process(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    fee: u32,
    tick_spacing: u32,
    initial_sqrt_price_x96: u128,
) -> ProgramResult {
    msg!("Initializing CLMM pool...");

    let account_info_iter = &mut accounts.iter();

    // Parse accounts
    let payer = next_account_info(account_info_iter)?;
    let pool_account = next_account_info(account_info_iter)?;
    let token_a_mint = next_account_info(account_info_iter)?;
    let token_b_mint = next_account_info(account_info_iter)?;
    let vault_a = next_account_info(account_info_iter)?;
    let vault_b = next_account_info(account_info_iter)?;
    let pool_authority = next_account_info(account_info_iter)?;
    let token_program = next_account_info(account_info_iter)?;
    let system_program = next_account_info(account_info_iter)?;
    let rent_sysvar = next_account_info(account_info_iter)?;

    // Validate payer is signer
    assert_signer(payer)?;

    // Validate token program
    if token_program.key.to_bytes() != spl_token::id().to_bytes() {
        msg!("Invalid token program");
        return Err(ProgramError::IncorrectProgramId);
    }

    // Validate system program
    if system_program.key != &ID {
        msg!("Invalid system program");
        return Err(ProgramError::IncorrectProgramId);
    }

    // Ensure tokens are sorted (token_a < token_b)
    let (token_0, token_1) = if token_a_mint.key < token_b_mint.key {
        (token_a_mint.key, token_b_mint.key)
    } else {
        (token_b_mint.key, token_a_mint.key)
    };

    // Validate pool PDA
    let (expected_pool_address, pool_bump) = derive_pool_address(
        program_id,
        token_0,
        token_1,
        fee,
    );

    if pool_account.key != &expected_pool_address {
        msg!("Invalid pool PDA");
        return Err(ProgramError::InvalidSeeds);
    }

    // Validate vault A PDA
    let (expected_vault_a, vault_a_bump) = derive_pool_vault_a_address(
        program_id,
        pool_account.key,
    );

    if vault_a.key != &expected_vault_a {
        msg!("Invalid vault A PDA");
        return Err(ProgramError::InvalidSeeds);
    }

    // Validate vault B PDA
    let (expected_vault_b, vault_b_bump) = derive_pool_vault_b_address(
        program_id,
        pool_account.key,
    );

    if vault_b.key != &expected_vault_b {
        msg!("Invalid vault B PDA");
        return Err(ProgramError::InvalidSeeds);
    }

    // Validate pool authority PDA
    let (expected_authority, _authority_bump) = derive_pool_authority_address(
        program_id,
        pool_account.key,
    );

    if pool_authority.key != &expected_authority {
        msg!("Invalid pool authority PDA");
        return Err(ProgramError::InvalidSeeds);
    }

    // Validate fee tier
    if fee > 10000 {
        msg!("Fee must be <= 10000 basis points (100%)");
        return Err(CLMMError::InvalidPrice.into());
    }

    // Validate tick spacing
    if tick_spacing == 0 || tick_spacing > 1000 {
        msg!("Invalid tick spacing");
        return Err(CLMMError::InvalidTickRange.into());
    }

    // Validate initial sqrt price
    let initial_sqrt_price = U256::from(initial_sqrt_price_x96);
    if initial_sqrt_price == U256::from(0) {
        msg!("Initial sqrt price cannot be zero");
        return Err(CLMMError::InvalidPrice.into());
    }

    // Get rent
    let rent = Rent::get()?;

    // Create pool account
    let pool_seeds = &[
        b"pool",
        token_0.as_ref(),
        token_1.as_ref(),
        &fee.to_le_bytes(),
        &[pool_bump],
    ];

    let pool_size = std::mem::size_of::<Pool>() + 8; // Add 8 for discriminator

    create_account(
        payer,
        pool_account,
        system_program,
        program_id,
        &rent,
        pool_size,
        pool_seeds,
    )?;

    // Create vault A (token account for token A)
    let vault_a_seeds = &[
        b"pool_vault",
        pool_account.key.as_ref(),
        b"a",
        &[vault_a_bump],
    ];

    create_account(
        payer,
        vault_a,
        system_program,
        program_id,
        &rent,
        165, // spl_token::state::Account::LEN
        vault_a_seeds,
    )?;

    // Initialize vault A as token account
    token_initialize_account(
        token_program,
        vault_a,
        token_a_mint,
        pool_authority,
        rent_sysvar,
    )?;

    // Create vault B (token account for token B)
    let vault_b_seeds = &[
        b"pool_vault",
        pool_account.key.as_ref(),
        b"b",
        &[vault_b_bump],
    ];

    create_account(
        payer,
        vault_b,
        system_program,
        program_id,
        &rent,
        165, // spl_token::state::Account::LEN
        vault_b_seeds,
    )?;

    // Initialize vault B as token account
    token_initialize_account(
        token_program,
        vault_b,
        token_b_mint,
        pool_authority,
        rent_sysvar,
    )?;

    // Create the pool state
    let pool = Pool::new(
        *token_0,
        *token_1,
        fee,
        tick_spacing,
        initial_sqrt_price,
    ).map_err(|e| {
        msg!("Failed to create pool: {}", e);
        CLMMError::InvalidPrice
    })?;

    // Validate pool
    if !pool.is_valid() {
        msg!("Invalid pool configuration");
        return Err(CLMMError::InvalidAccount.into());
    }

    // Write pool data to account
    write_account_data(pool_account, &pool)?;

    msg!("Pool initialized successfully");
    msg!("  Token A: {}", token_0);
    msg!("  Token B: {}", token_1);
    msg!("  Fee: {} bps", fee);
    msg!("  Tick spacing: {}", tick_spacing);
    msg!("  Initial sqrt price: {}", initial_sqrt_price);
    msg!("  Initial tick: {}", pool.tick);
    msg!("  Pool authority: {}", pool_authority.key);
    msg!("  Vault A: {}", vault_a.key);
    msg!("  Vault B: {}", vault_b.key);

    Ok(())
}
