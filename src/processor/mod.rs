use solana_program::{
    account_info::AccountInfo,
    entrypoint::ProgramResult,
    msg,
    program_error::ProgramError,
    pubkey::Pubkey,
};
use borsh::BorshDeserialize;

pub mod swap;
pub mod initialize_pool;
pub mod add_liquidity;
pub mod remove_liquidity;
pub mod collect_fees;

/// Instructions supported by the CLMM program
#[derive(BorshDeserialize, Debug)]
pub enum CLMMInstruction {
    /// Initialize a new pool
    ///
    /// Accounts expected:
    /// 0. `[signer]` Payer
    /// 1. `[writable]` Pool account (PDA)
    /// 2. `[]` Token A mint
    /// 3. `[]` Token B mint
    /// 4. `[writable]` Pool vault A (PDA)
    /// 5. `[writable]` Pool vault B (PDA)
    /// 6. `[]` Pool authority (PDA)
    /// 7. `[]` Token program
    /// 8. `[]` System program
    /// 9. `[]` Rent sysvar
    ///
    /// Data:
    /// - fee: u32 (in basis points, e.g., 30 = 0.30%)
    /// - tick_spacing: u32
    /// - initial_sqrt_price_x96: u128
    InitializePool {
        fee: u32,
        tick_spacing: u32,
        initial_sqrt_price_x96: u128,
    },

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
    ///
    /// Data:
    /// - tick_lower: i32
    /// - tick_upper: i32
    /// - liquidity_delta: u128
    /// - amount_0_max: u64
    /// - amount_1_max: u64
    AddLiquidity {
        tick_lower: i32,
        tick_upper: i32,
        liquidity_delta: u128,
        amount_0_max: u64,
        amount_1_max: u64,
    },

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
    ///
    /// Data:
    /// - liquidity_delta: u128
    /// - amount_0_min: u64
    /// - amount_1_min: u64
    RemoveLiquidity {
        liquidity_delta: u128,
        amount_0_min: u64,
        amount_1_min: u64,
    },

    /// Collect fees from a position
    ///
    /// Accounts expected:
    /// 0. `[signer]` Position owner
    /// 1. `[writable]` Pool account
    /// 2. `[writable]` Position account
    /// 3. `[writable]` User token A account
    /// 4. `[writable]` User token B account
    /// 5. `[writable]` Pool vault A
    /// 6. `[writable]` Pool vault B
    /// 7. `[]` Pool authority (PDA)
    /// 8. `[]` Token program
    ///
    /// Data:
    /// - amount_0_requested: u64 (0 = collect all)
    /// - amount_1_requested: u64 (0 = collect all)
    CollectFees {
        amount_0_requested: u64,
        amount_1_requested: u64,
    },

    /// Execute a swap
    ///
    /// Accounts expected:
    /// 0. `[signer]` User account
    /// 1. `[writable]` Pool account
    /// 2. `[writable]` User token A account
    /// 3. `[writable]` User token B account
    /// 4. `[writable]` Pool vault A
    /// 5. `[writable]` Pool vault B
    /// 6. `[]` Pool authority (PDA)
    /// 7. `[]` Token program
    ///
    /// Data:
    /// - amount_in: u64
    /// - minimum_amount_out: u64
    /// - sqrt_price_limit: u128
    /// - zero_for_one: bool
    Swap {
        amount_in: u64,
        minimum_amount_out: u64,
        sqrt_price_limit: u128,
        zero_for_one: bool,
    },
}

/// Main processor function that dispatches to specific instruction handlers
pub fn process<'a>(
    program_id: &Pubkey,
    accounts: &'a [AccountInfo<'a>],
    instruction_data: &[u8],
) -> ProgramResult {
    // Deserialize the instruction
    let instruction = CLMMInstruction::try_from_slice(instruction_data)
        .map_err(|_| {
            msg!("Failed to deserialize instruction");
            ProgramError::InvalidInstructionData
        })?;

    msg!("Processing instruction: {:?}", instruction);

    // Dispatch to the appropriate processor
    match instruction {
        CLMMInstruction::InitializePool {
            fee,
            tick_spacing,
            initial_sqrt_price_x96,
        } => {
            msg!("Instruction: InitializePool");
            initialize_pool::process(
                program_id,
                accounts,
                fee,
                tick_spacing,
                initial_sqrt_price_x96,
            )
        }

        CLMMInstruction::AddLiquidity {
            tick_lower,
            tick_upper,
            liquidity_delta,
            amount_0_max,
            amount_1_max,
        } => {
            msg!("Instruction: AddLiquidity");
            add_liquidity::process(
                program_id,
                accounts,
                tick_lower,
                tick_upper,
                liquidity_delta,
                amount_0_max,
                amount_1_max,
            )
        }

        CLMMInstruction::RemoveLiquidity {
            liquidity_delta,
            amount_0_min,
            amount_1_min,
        } => {
            msg!("Instruction: RemoveLiquidity");
            remove_liquidity::process(
                program_id,
                accounts,
                liquidity_delta,
                amount_0_min,
                amount_1_min,
            )
        }

        CLMMInstruction::CollectFees {
            amount_0_requested,
            amount_1_requested,
        } => {
            msg!("Instruction: CollectFees");
            collect_fees::process(
                program_id,
                accounts,
                amount_0_requested,
                amount_1_requested,
            )
        }

        CLMMInstruction::Swap {
            amount_in,
            minimum_amount_out,
            sqrt_price_limit,
            zero_for_one: _zero_for_one,
        } => {
            msg!("Instruction: Swap");
            swap::process(
                accounts,
                amount_in,
                minimum_amount_out,
                sqrt_price_limit,
            )
        }
    }
}
