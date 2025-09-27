// Instruction dispatch
use solana_program::{
    account_info::AccountInfo,
    entrypoint::ProgramResult,
    pubkey::Pubkey,
};

pub mod swap;

/// Main processor function that dispatches to specific instruction handlers
pub fn process(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> ProgramResult {
    // For now, we'll dispatch to the swap processor
    // In a complete implementation, we'd decode the instruction type first
    swap::process(accounts, 0, 0, 0)
}
