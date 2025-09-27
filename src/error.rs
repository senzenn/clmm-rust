use solana_program::program_error::ProgramError;
use thiserror::Error;

#[derive(Error, Debug, Copy, Clone)]
pub enum CLMMError {
    #[error("Invalid instruction")]
    InvalidInstruction,

    #[error("Invalid account")]
    InvalidAccount,

    #[error("Math overflow")]
    MathOverflow,

    #[error("Invalid tick range")]
    InvalidTickRange,

    #[error("Insufficient liquidity")]
    InsufficientLiquidity,

    #[error("Invalid price")]
    InvalidPrice,

    #[error("Unauthorized")]
    Unauthorized,
}

impl From<CLMMError> for ProgramError {
    fn from(e: CLMMError) -> Self {
        ProgramError::Custom(e as u32)
    }
}
