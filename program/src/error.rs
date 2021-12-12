//! Error types.
use num_derive::FromPrimitive;
use num_traits::FromPrimitive;
use solana_program::{
    decode_error::DecodeError, program_error::PrintProgramError, program_error::ProgramError,
};
use thiserror::Error;

/// Errors that may be returned by the Collateral program.
#[derive(Clone, Debug, Eq, Error, FromPrimitive, PartialEq)]
pub enum LendingPlatformError {
    /// Invalid instruction.
    #[error("Invalid Instruction")]
    InvalidInstruction,

    #[error("Incorrect signers for the instruction.")]
    IncorrectSigner,

    #[error("account must be rent-exempt.")]
    NotRentExempt,

    #[error("NFT state/set account can only be initialized once.")]
    AlreadyInitialized,

    #[error("Invalid accounts passed in the instruction")]
    InvalidAccounts,

    #[error("Overflow when calculating")]
    MathOverFlow,

    #[error("Loan is still valid, so cannot be defaulted")]
    StillValidLoan,

    #[error("Loan is already defaulted")]
    DefaultedLoan,

    #[error("Lending pool is not open")]
    LendingPoolNotOpen,

    #[error("Invalid SOL to USD feed")]
    InvalidSOLUSDFeed,

    #[error("Lending pool does not have enough funds")]
    NotEnoughFundsInLendingPool,

    #[error("Lending pool has outstanding loans")]
    LendingPoolHasOutstandingLoan
}

impl From<LendingPlatformError> for ProgramError {
    fn from(e: LendingPlatformError) -> Self {
        ProgramError::Custom(e as u32)
    }
}

impl<T> DecodeError<T> for LendingPlatformError {
    fn type_of() -> &'static str {
        "NFT Error"
    }
}

impl PrintProgramError for LendingPlatformError {
    fn print<E>(&self)
    where
        E: 'static + std::error::Error + DecodeError<E> + PrintProgramError + FromPrimitive,
    {
        match self {
            LendingPlatformError::InvalidInstruction => println!("Error: Invalid instruction."),
            LendingPlatformError::IncorrectSigner => {
                println!("Error: Incorrect signers for the instruction.")
            }
            LendingPlatformError::NotRentExempt => {
                println!("account must be rent-exempt.")
            }
            LendingPlatformError::AlreadyInitialized => {
                println!("NFT state can only be initialized once.")
            }
            LendingPlatformError::InvalidAccounts => {
                println!("Invalid accounts passed in the instruction")
            },
            LendingPlatformError::DefaultedLoan => {
                println!("Loan is already defaulted")
            }
            LendingPlatformError::MathOverFlow => {
                println!("Overflow when calculating")
            }
            LendingPlatformError::StillValidLoan => {
                println!("Loan is still valid, so cannot be defaulted")
            }
            LendingPlatformError::LendingPoolNotOpen => {
                println!("Lending pool is not open")
            }
            LendingPlatformError::InvalidSOLUSDFeed => {
                println!("Invalid SOL to USD chainlink feed account passed")
            }
            LendingPlatformError::NotEnoughFundsInLendingPool => {
                println!("Not enough funds in lending pool")
            }
            LendingPlatformError::LendingPoolHasOutstandingLoan => {
                println!("Lending pool has outstanding loans")
            }
        }
    }
}
