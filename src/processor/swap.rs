use solana_program::{
    account_info::{next_account_info, AccountInfo},
    entrypoint::ProgramResult,
    instruction::{AccountMeta, Instruction},
    program::invoke,
    pubkey::Pubkey,
};
use borsh::{BorshDeserialize, BorshSerialize};
use crate::error::CLMMError;
use crate::math::SwapEngine;
use crate::state::Pool;
use std::collections::VecDeque;

/// Swap processor for handling swap instructions
pub struct SwapProcessor;

/// Process swap instruction
pub fn process<'a>(
    accounts: &'a [AccountInfo<'a>],
    amount_in: u64,
    minimum_amount_out: u64,
    sqrt_price_limit: u128,
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    let pool_account = next_account_info(account_info_iter)?;
    let user_account = next_account_info(account_info_iter)?;
    let user_token_a_account = next_account_info(account_info_iter)?;
    let user_token_b_account = next_account_info(account_info_iter)?;
    let pool_token_a_vault = next_account_info(account_info_iter)?;
    let pool_token_b_vault = next_account_info(account_info_iter)?;
    let token_program = next_account_info(account_info_iter)?;

    // Deserialize pool state
    let pool_data = &pool_account.data.borrow();
    let mut pool_slice = pool_data.as_ref();
    let mut pool = Pool::deserialize(&mut pool_slice)?;

    // Validate accounts
    if !pool_account.is_writable {
        return Err(CLMMError::InvalidAccount.into());
    }
    if !user_token_a_account.is_writable {
        return Err(CLMMError::InvalidAccount.into());
    }
    if !user_token_b_account.is_writable {
        return Err(CLMMError::InvalidAccount.into());
    }
    if !pool_token_a_vault.is_writable {
        return Err(CLMMError::InvalidAccount.into());
    }
    if !pool_token_b_vault.is_writable {
        return Err(CLMMError::InvalidAccount.into());
    }

    // Add proper token account validation
    SwapProcessor::validate_token_accounts(&pool, pool_account, user_token_a_account, user_token_b_account, pool_token_a_vault, pool_token_b_vault)?;

    // Add signer validation
    if !user_account.is_signer {
        return Err(CLMMError::Unauthorized.into());
    }

    let amount_in_u256 = crate::math::tick_math::U256::from(amount_in);
    let sqrt_price_limit_u256 = crate::math::tick_math::U256::from(sqrt_price_limit);
    let minimum_amount_out_u256 = crate::math::tick_math::U256::from(minimum_amount_out);

    // Determine swap direction (simplified - in real implementation would check token accounts)
    let zero_for_one = true; // Assume token0 -> token1 for now

    // Execute the swap with dynamic fee adjustment
    let mut price_history = VecDeque::new();
    let mut volume_history = VecDeque::new();
    let mut impact_history = VecDeque::new();
    let current_timestamp = 1000; // TODO: Get actual timestamp from instruction context

    let mut oracle_observations = VecDeque::new();
    let swap_result = SwapEngine::execute_swap(
        &mut pool,
        amount_in_u256,
        zero_for_one,
        sqrt_price_limit_u256,
        &user_account.key,
        &mut price_history,
        &mut volume_history,
        &mut impact_history,
        &mut oracle_observations,
        current_timestamp,
        1, // TODO: Get actual sequence number from instruction context
    )?;

    // Validate minimum output
    if swap_result.amount_out < minimum_amount_out_u256 {
        return Err(CLMMError::InsufficientLiquidity.into());
    }

    // Validate price impact (optional - could be a parameter)
    let max_price_impact = 100; // 1% max impact
    if swap_result.price_impact > max_price_impact {
        return Err(CLMMError::InvalidPrice.into());
    }

    // Update pool account data
    pool.serialize(&mut &mut pool_account.data.borrow_mut()[..])?;

    // Transfer tokens between accounts
    SwapProcessor::transfer_tokens(
        token_program,
        user_account,
        user_token_a_account,
        pool_token_a_vault,
        user_token_b_account,
        pool_token_b_vault,
        swap_result.amount_in.low_u128() as u64,
        swap_result.amount_out.low_u128() as u64,
        zero_for_one,
    )?;

    Ok(())
}

impl SwapProcessor {
    /// Validate token accounts for swap operation
    fn validate_token_accounts(
        _pool: &Pool,
        _pool_account: &AccountInfo,
        user_token_a_account: &AccountInfo,
        user_token_b_account: &AccountInfo,
        pool_token_a_vault: &AccountInfo,
        pool_token_b_vault: &AccountInfo,
    ) -> ProgramResult {
        // Validate that user token accounts are owned by the token program
        if user_token_a_account.owner.to_bytes() != spl_token::id().to_bytes() {
            return Err(CLMMError::InvalidAccount.into());
        }
        if user_token_b_account.owner.to_bytes() != spl_token::id().to_bytes() {
            return Err(CLMMError::InvalidAccount.into());
        }
        if pool_token_a_vault.owner.to_bytes() != spl_token::id().to_bytes() {
            return Err(CLMMError::InvalidAccount.into());
        }
        if pool_token_b_vault.owner.to_bytes() != spl_token::id().to_bytes() {
            return Err(CLMMError::InvalidAccount.into());
        }

        // Basic validation - in a real implementation, you'd validate token accounts properly
        // For now, just ensure the accounts are different and properly sized
        if user_token_a_account.key == user_token_b_account.key {
            return Err(CLMMError::InvalidAccount.into());
        }
        if pool_token_a_vault.key == pool_token_b_vault.key {
            return Err(CLMMError::InvalidAccount.into());
        }

        // Additional validation would require proper token program integration
        // This is a simplified version for compilation purposes

        Ok(())
    }

    /// Transfer tokens between accounts for swap operation
    fn transfer_tokens<'a>(
        token_program: &'a AccountInfo<'a>,
        authority: &'a AccountInfo<'a>,
        user_token_a_account: &'a AccountInfo<'a>,
        pool_token_a_vault: &'a AccountInfo<'a>,
        user_token_b_account: &'a AccountInfo<'a>,
        pool_token_b_vault: &'a AccountInfo<'a>,
        amount_in: u64,
        amount_out: u64,
        zero_for_one: bool,
    ) -> ProgramResult {
        if zero_for_one {
            // Transfer token0 from user to pool vault
            SwapProcessor::token_transfer_cpi(
                token_program,
                user_token_a_account,
                pool_token_a_vault,
                authority,
                amount_in,
            )?;

            // Transfer token1 from pool vault to user
            SwapProcessor::token_transfer_cpi(
                token_program,
                pool_token_b_vault,
                user_token_b_account,
                authority,
                amount_out,
            )?;
        } else {
            // Transfer token1 from user to pool vault
            SwapProcessor::token_transfer_cpi(
                token_program,
                user_token_b_account,
                pool_token_b_vault,
                authority,
                amount_in,
            )?;

            // Transfer token0 from pool vault to user
            SwapProcessor::token_transfer_cpi(
                token_program,
                pool_token_a_vault,
                user_token_a_account,
                authority,
                amount_out,
            )?;
        }

        Ok(())
    }

    /// Execute token transfer CPI call
    fn token_transfer_cpi<'a>(
        token_program: &'a AccountInfo<'a>,
        from: &'a AccountInfo<'a>,
        to: &'a AccountInfo<'a>,
        authority: &'a AccountInfo<'a>,
        amount: u64,
    ) -> ProgramResult {
        // SPL Token transfer instruction discriminator
        const TOKEN_IX_TRANSFER: u8 = 3;

        // Helper function to get the SPL Token program ID as our Pubkey type
        fn token_program_id() -> Pubkey {
            Pubkey::new_from_array(spl_token::id().to_bytes())
        }

        let mut data = Vec::with_capacity(9);
        data.push(TOKEN_IX_TRANSFER);
        data.extend_from_slice(&amount.to_le_bytes());

        let ix = Instruction {
            program_id: token_program_id(),
            accounts: vec![
                AccountMeta::new(*from.key, false),
                AccountMeta::new(*to.key, false),
                AccountMeta::new_readonly(*authority.key, true),
            ],
            data,
        };

        invoke(
            &ix,
            &[
                from.clone(),
                to.clone(),
                authority.clone(),
                token_program.clone(),
            ],
        )
    }
}

