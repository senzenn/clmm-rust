use solana_program::{
    account_info::AccountInfo,
    entrypoint,
    entrypoint::ProgramResult,
    pubkey::Pubkey,
};

pub mod error;
pub mod instruction;
pub mod math;
pub mod processor;
pub mod state;
pub mod utils;

// Re-export commonly used types
pub use math::*;
pub use state::*;
pub use error::*;

// Declare the program's entrypoint
entrypoint!(process_instruction);

/// The entrypoint to our Solana program
pub fn process_instruction<'a>(
    program_id: &Pubkey,
    accounts: &'a [AccountInfo<'a>],
    instruction_data: &[u8],
) -> ProgramResult {
    processor::process(program_id, accounts, instruction_data)
}
