use solana_program::{
    account_info::AccountInfo,
    entrypoint::ProgramResult,
    instruction::{AccountMeta, Instruction},
    program::{invoke, invoke_signed},
    program_error::ProgramError,
    pubkey::Pubkey,
};

// SPL Token instruction discriminators
const TOKEN_IX_TRANSFER: u8 = 3;
const TOKEN_IX_MINT_TO: u8 = 7;
const TOKEN_IX_BURN: u8 = 8;
const TOKEN_IX_CLOSE_ACCOUNT: u8 = 9;
const TOKEN_IX_INITIALIZE_ACCOUNT: u8 = 1;

// Helper function to get the SPL Token program ID as our Pubkey type
fn token_program_id() -> Pubkey {
    Pubkey::new_from_array(spl_token::id().to_bytes())
}

/// Transfer SPL tokens from one account to another
pub fn token_transfer<'a>(
    token_program: &AccountInfo<'a>,
    source: &AccountInfo<'a>,
    destination: &AccountInfo<'a>,
    authority: &AccountInfo<'a>,
    amount: u64,
) -> ProgramResult {
    let mut data = Vec::with_capacity(9);
    data.push(TOKEN_IX_TRANSFER);
    data.extend_from_slice(&amount.to_le_bytes());

    let ix = Instruction {
        program_id: token_program_id(),
        accounts: vec![
            AccountMeta::new(*source.key, false),
            AccountMeta::new(*destination.key, false),
            AccountMeta::new_readonly(*authority.key, true),
        ],
        data,
    };

    invoke(
        &ix,
        &[
            source.clone(),
            destination.clone(),
            authority.clone(),
            token_program.clone(),
        ],
    )
}

/// Transfer SPL tokens using PDA authority with seeds
pub fn token_transfer_signed<'a>(
    token_program: &AccountInfo<'a>,
    source: &AccountInfo<'a>,
    destination: &AccountInfo<'a>,
    authority: &AccountInfo<'a>,
    amount: u64,
    signer_seeds: &[&[u8]],
) -> ProgramResult {
    let mut data = Vec::with_capacity(9);
    data.push(TOKEN_IX_TRANSFER);
    data.extend_from_slice(&amount.to_le_bytes());

    let ix = Instruction {
        program_id: token_program_id(),
        accounts: vec![
            AccountMeta::new(*source.key, false),
            AccountMeta::new(*destination.key, false),
            AccountMeta::new_readonly(*authority.key, true),
        ],
        data,
    };

    invoke_signed(
        &ix,
        &[
            source.clone(),
            destination.clone(),
            authority.clone(),
            token_program.clone(),
        ],
        &[signer_seeds],
    )
}

/// Mint SPL tokens to a destination account
pub fn token_mint_to<'a>(
    token_program: &AccountInfo<'a>,
    mint: &AccountInfo<'a>,
    destination: &AccountInfo<'a>,
    authority: &AccountInfo<'a>,
    amount: u64,
    signer_seeds: &[&[u8]],
) -> ProgramResult {
    let mut data = Vec::with_capacity(9);
    data.push(TOKEN_IX_MINT_TO);
    data.extend_from_slice(&amount.to_le_bytes());

    let ix = Instruction {
        program_id: token_program_id(),
        accounts: vec![
            AccountMeta::new(*mint.key, false),
            AccountMeta::new(*destination.key, false),
            AccountMeta::new_readonly(*authority.key, true),
        ],
        data,
    };

    invoke_signed(
        &ix,
        &[
            mint.clone(),
            destination.clone(),
            authority.clone(),
            token_program.clone(),
        ],
        &[signer_seeds],
    )
}

/// Burn SPL tokens from an account
pub fn token_burn<'a>(
    token_program: &AccountInfo<'a>,
    account: &AccountInfo<'a>,
    mint: &AccountInfo<'a>,
    authority: &AccountInfo<'a>,
    amount: u64,
) -> ProgramResult {
    let mut data = Vec::with_capacity(9);
    data.push(TOKEN_IX_BURN);
    data.extend_from_slice(&amount.to_le_bytes());

    let ix = Instruction {
        program_id: token_program_id(),
        accounts: vec![
            AccountMeta::new(*account.key, false),
            AccountMeta::new(*mint.key, false),
            AccountMeta::new_readonly(*authority.key, true),
        ],
        data,
    };

    invoke(
        &ix,
        &[
            account.clone(),
            mint.clone(),
            authority.clone(),
            token_program.clone(),
        ],
    )
}

/// Initialize a new SPL token account
pub fn token_initialize_account<'a>(
    token_program: &AccountInfo<'a>,
    account: &AccountInfo<'a>,
    mint: &AccountInfo<'a>,
    owner: &AccountInfo<'a>,
    rent: &AccountInfo<'a>,
) -> ProgramResult {
    let ix = Instruction {
        program_id: token_program_id(),
        accounts: vec![
            AccountMeta::new(*account.key, false),
            AccountMeta::new_readonly(*mint.key, false),
            AccountMeta::new_readonly(*owner.key, false),
            AccountMeta::new_readonly(*rent.key, false),
        ],
        data: vec![TOKEN_IX_INITIALIZE_ACCOUNT],
    };

    invoke(
        &ix,
        &[
            account.clone(),
            mint.clone(),
            owner.clone(),
            rent.clone(),
            token_program.clone(),
        ],
    )
}

/// Close an SPL token account
pub fn token_close_account<'a>(
    token_program: &AccountInfo<'a>,
    account: &AccountInfo<'a>,
    destination: &AccountInfo<'a>,
    authority: &AccountInfo<'a>,
    signer_seeds: &[&[u8]],
) -> ProgramResult {
    let ix = Instruction {
        program_id: token_program_id(),
        accounts: vec![
            AccountMeta::new(*account.key, false),
            AccountMeta::new(*destination.key, false),
            AccountMeta::new_readonly(*authority.key, true),
        ],
        data: vec![TOKEN_IX_CLOSE_ACCOUNT],
    };

    invoke_signed(
        &ix,
        &[
            account.clone(),
            destination.clone(),
            authority.clone(),
            token_program.clone(),
        ],
        &[signer_seeds],
    )
}

/// Get the balance of an SPL token account
pub fn get_token_balance(account: &AccountInfo) -> Result<u64, ProgramError> {
    let data = account.try_borrow_data()?;
    if data.len() != 165 {
        return Err(ProgramError::InvalidAccountData);
    }

    let amount_bytes = &data[64..72];
    let amount = u64::from_le_bytes(amount_bytes.try_into().map_err(|_| ProgramError::InvalidAccountData)?);
    Ok(amount)
}

/// Verify that an account is owned by the SPL Token program
pub fn assert_is_token_account(account: &AccountInfo) -> ProgramResult {
    if account.owner.to_bytes() != spl_token::id().to_bytes() {
        return Err(ProgramError::IllegalOwner);
    }
    Ok(())
}

/// Verify that a token account's mint matches the expected mint
pub fn assert_token_mint(
    token_account: &AccountInfo,
    expected_mint: &Pubkey,
) -> ProgramResult {
    assert_is_token_account(token_account)?;

    let data = token_account.try_borrow_data()?;
    if data.len() < 32 {
        return Err(ProgramError::InvalidAccountData);
    }

    let mint_bytes = &data[0..32];
    if mint_bytes != expected_mint.to_bytes() {
        return Err(ProgramError::InvalidAccountData);
    }

    Ok(())
}

/// Verify that a token account's owner matches the expected owner
pub fn assert_token_owner(
    token_account: &AccountInfo,
    expected_owner: &Pubkey,
) -> ProgramResult {
    assert_is_token_account(token_account)?;

    let data = token_account.try_borrow_data()?;
    if data.len() < 64 {
        return Err(ProgramError::InvalidAccountData);
    }

    let owner_bytes = &data[32..64];
    if owner_bytes != expected_owner.to_bytes() {
        return Err(ProgramError::InvalidAccountData);
    }

    Ok(())
}
