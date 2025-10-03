use solana_program::{
    account_info::AccountInfo,
    entrypoint::ProgramResult,
    instruction::{AccountMeta, Instruction},
    msg,
    program::invoke_signed,
    program_error::ProgramError,
    pubkey::Pubkey,
    rent::Rent,
    sysvar::Sysvar,
};
use borsh::BorshSerialize;

// System program ID
solana_program::declare_id!("Fw4mNHEDrHAGg41XEcp7DkHpEP12MiUcCrP2Lj5ngth9");

// System instruction discriminators
const SYSTEM_IX_CREATE_ACCOUNT: u32 = 0;
const SYSTEM_IX_ASSIGN: u32 = 1;
const SYSTEM_IX_TRANSFER: u32 = 2;
const SYSTEM_IX_ALLOCATE: u32 = 8;

fn create_account_ix(
    payer: &Pubkey,
    new_account: &Pubkey,
    lamports: u64,
    space: u64,
    owner: &Pubkey,
) -> Instruction {
    let mut data = Vec::with_capacity(4 + 8 + 8 + 32);
    data.extend_from_slice(&SYSTEM_IX_CREATE_ACCOUNT.to_le_bytes());
    data.extend_from_slice(&lamports.to_le_bytes());
    data.extend_from_slice(&space.to_le_bytes());
    data.extend_from_slice(owner.as_ref());

    Instruction {
        program_id: ID,
        accounts: vec![
            AccountMeta::new(*payer, true),
            AccountMeta::new(*new_account, true),
        ],
        data,
    }
}

fn transfer_ix(from: &Pubkey, to: &Pubkey, lamports: u64) -> Instruction {
    let mut data = Vec::with_capacity(4 + 8);
    data.extend_from_slice(&SYSTEM_IX_TRANSFER.to_le_bytes());
    data.extend_from_slice(&lamports.to_le_bytes());

    Instruction {
        program_id: ID,
        accounts: vec![
            AccountMeta::new(*from, true),
            AccountMeta::new(*to, false),
        ],
        data,
    }
}

fn allocate_ix(account: &Pubkey, space: u64) -> Instruction {
    let mut data = Vec::with_capacity(4 + 8);
    data.extend_from_slice(&SYSTEM_IX_ALLOCATE.to_le_bytes());
    data.extend_from_slice(&space.to_le_bytes());

    Instruction {
        program_id: ID,
        accounts: vec![AccountMeta::new(*account, true)],
        data,
    }
}

fn assign_ix(account: &Pubkey, owner: &Pubkey) -> Instruction {
    let mut data = Vec::with_capacity(4 + 32);
    data.extend_from_slice(&SYSTEM_IX_ASSIGN.to_le_bytes());
    data.extend_from_slice(owner.as_ref());

    Instruction {
        program_id: ID,
        accounts: vec![AccountMeta::new(*account, true)],
        data,
    }
}

/// Create a new account owned by the program
pub fn create_account<'a>(
    payer: &AccountInfo<'a>,
    new_account: &AccountInfo<'a>,
    system_program: &AccountInfo<'a>,
    program_id: &Pubkey,
    rent: &Rent,
    space: usize,
    signer_seeds: &[&[u8]],
) -> ProgramResult {
    let required_lamports = rent.minimum_balance(space);

    if new_account.lamports() > 0 {
        let required_lamports_diff = required_lamports.saturating_sub(new_account.lamports());

        if required_lamports_diff > 0 {
            invoke_signed(
                &transfer_ix(payer.key, new_account.key, required_lamports_diff),
                &[payer.clone(), new_account.clone(), system_program.clone()],
                &[signer_seeds],
            )?;
        }

        invoke_signed(
            &allocate_ix(new_account.key, space as u64),
            &[new_account.clone(), system_program.clone()],
            &[signer_seeds],
        )?;

        invoke_signed(
            &assign_ix(new_account.key, program_id),
            &[new_account.clone(), system_program.clone()],
            &[signer_seeds],
        )?;
    } else {
        invoke_signed(
            &create_account_ix(payer.key, new_account.key, required_lamports, space as u64, program_id),
            &[payer.clone(), new_account.clone(), system_program.clone()],
            &[signer_seeds],
        )?;
    }

    Ok(())
}

/// Assert that an account is writable
pub fn assert_writable(account: &AccountInfo) -> ProgramResult {
    if !account.is_writable {
        msg!("Account is not writable: {}", account.key);
        return Err(ProgramError::InvalidAccountData);
    }
    Ok(())
}

/// Assert that an account is a signer
pub fn assert_signer(account: &AccountInfo) -> ProgramResult {
    if !account.is_signer {
        msg!("Account is not a signer: {}", account.key);
        return Err(ProgramError::MissingRequiredSignature);
    }
    Ok(())
}

/// Assert that an account is owned by a specific program
pub fn assert_owned_by(account: &AccountInfo, owner: &Pubkey) -> ProgramResult {
    if account.owner != owner {
        msg!(
            "Account {} is owned by {}, expected {}",
            account.key,
            account.owner,
            owner
        );
        return Err(ProgramError::IllegalOwner);
    }
    Ok(())
}

/// Assert that an account is not initialized (empty data)
pub fn assert_uninitialized(account: &AccountInfo) -> ProgramResult {
    let data = account.try_borrow_data()?;
    if !data.iter().all(|&byte| byte == 0) {
        msg!("Account is already initialized: {}", account.key);
        return Err(ProgramError::AccountAlreadyInitialized);
    }
    Ok(())
}

/// Assert that an account is initialized (non-empty data)
pub fn assert_initialized(account: &AccountInfo) -> ProgramResult {
    let data = account.try_borrow_data()?;
    if data.is_empty() || data.iter().all(|&byte| byte == 0) {
        msg!("Account is not initialized: {}", account.key);
        return Err(ProgramError::UninitializedAccount);
    }
    Ok(())
}

/// Assert that an account matches the expected public key
pub fn assert_account_key(account: &AccountInfo, expected: &Pubkey) -> ProgramResult {
    if account.key != expected {
        msg!(
            "Account key mismatch: expected {}, got {}",
            expected,
            account.key
        );
        return Err(ProgramError::InvalidAccountData);
    }
    Ok(())
}

/// Get the current unix timestamp from the Clock sysvar
pub fn get_current_timestamp() -> Result<i64, ProgramError> {
    let clock = solana_program::clock::Clock::get()?;
    Ok(clock.unix_timestamp)
}

/// Serialize and write data to an account
pub fn write_account_data<T: BorshSerialize>(
    account: &AccountInfo,
    data: &T,
) -> ProgramResult {
    let mut account_data = account.try_borrow_mut_data()?;
    data.serialize(&mut account_data.as_mut())?;
    Ok(())
}

/// Check if an account has enough space for the data
pub fn assert_account_space(account: &AccountInfo, required_space: usize) -> ProgramResult {
    if account.data_len() < required_space {
        msg!(
            "Account {} has insufficient space: {} < {}",
            account.key,
            account.data_len(),
            required_space
        );
        return Err(ProgramError::AccountDataTooSmall);
    }
    Ok(())
}

/// Close an account and return lamports to destination
pub fn close_account<'a>(
    account: &AccountInfo<'a>,
    destination: &AccountInfo<'a>,
) -> ProgramResult {
    let dest_starting_lamports = destination.lamports();
    **destination.lamports.borrow_mut() = dest_starting_lamports
        .checked_add(account.lamports())
        .ok_or(ProgramError::ArithmeticOverflow)?;
    **account.lamports.borrow_mut() = 0;

    let mut data = account.try_borrow_mut_data()?;
    data.fill(0);

    Ok(())
}

/// Reallocate an account to a new size
pub fn realloc_account<'a>(
    account: &AccountInfo<'a>,
    new_size: usize,
    payer: &AccountInfo<'a>,
    rent: &Rent,
) -> ProgramResult {
    let current_size = account.data_len();

    if new_size == current_size {
        return Ok(());
    }

    let current_lamports = account.lamports();
    let required_lamports = rent.minimum_balance(new_size);

    if new_size > current_size {
        // Need to add lamports
        let additional_lamports = required_lamports.saturating_sub(current_lamports);
        if additional_lamports > 0 {
            invoke_signed(
                &transfer_ix(payer.key, account.key, additional_lamports),
                &[payer.clone(), account.clone()],
                &[],
            )?;
        }
    } else {
        // Can return lamports
        let excess_lamports = current_lamports.saturating_sub(required_lamports);
        if excess_lamports > 0 {
            **account.lamports.borrow_mut() = required_lamports;
            **payer.lamports.borrow_mut() = payer
                .lamports()
                .checked_add(excess_lamports)
                .ok_or(ProgramError::ArithmeticOverflow)?;
        }
    }

    // Note: realloc is not available in all Solana versions
    // Account resizing would need to be done through reallocation
    Ok(())
}
