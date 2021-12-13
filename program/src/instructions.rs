use crate::error::LendingPlatformError;
use crate::params::{NewLendingPool, NewLoan, PaybackLoan, DefaultLoan, CloseLending, InitLendingPoolAccount, InitLoanAccount};
use solana_program::program_error::ProgramError;
use solana_program::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    sysvar,
};

#[derive(Debug, PartialEq)]
pub enum LendingPlatformInstructions {

    /// Initiates Loan pool pda account
    ///
    /// 0. `[]` system program
    /// 1. `[]` Rent sysvar account
    /// 2. `[]` lending pool pda account
    /// 3. `[signer]` borrower account
    /// 4. `[]` Empty loan account
    InitLoanAccount(InitLoanAccount),

    /// Initiates lending pool pda account.
    ///
    /// 0. `[]` System account
    /// 1. `[]` Rent sysvar account
    /// 2. `[singer]` Lender account
    /// 3. `[]` Lender spl account which must be owned by the lender
    /// 4. `[]` Empty lending pool pda account
    InitLendingPoolAccount(InitLendingPoolAccount),


    /// Creates new lending pool, works by 1. Initialize the lending pool account
    /// 2. Initialize empty spl token account 3 with owner set to 2 and mint to 4.
    /// 3. Transfer coin from 1 to 3
    ///
    /// 0. `[signer]` Lender who want to provide loan
    /// 1. `[writable]` Coin SPL token account owner must be 0 and mint must be 4.
    /// 2. `[writable]` Empty lending pool account owned by this program (PDA account) (rent exemption check)
    /// 3. `[writable]` empty SPL token account which will hold the lending amount and owner set to 2 (rent exemption check)
    /// 4. `[]` Chainlink feed account (must match feed account pubkey stored in lending pool account)
    /// 5. `[]` Coin SPL token mint
    /// 6. `[]` rent sysvar
    /// 7. `[]` token program
    NewLendingPool(NewLendingPool),

    /// Creates new loan
    /// This works by 1. Transferring SOLs from borrower to Loan PDA 2. Transfer stable coins from lending PDA
    /// SPL to Loan PDA SPL 3. Transfer stable coins from loan pda spl to Borrower's spl
    /// 0. `[]` Lender spl account (for lending pool verification)
    /// 1. `[signer]` Borrower who wants to take loan
    /// 2. `[]` SPL token account in which borrower will receive the loan
    /// 3. `[writable]` Lending pool account which is initialized and owned by this program (PDA account)
    /// 4. `[]` Lending pool PDA controlled SPL token account from which funds will transfer to borrower
    /// 5. `[]` Lending pool owner's SPL token account
    /// 6. `[writable]` Empty loan account owned by this program (PDA account)
    /// 7. `[]` Empty Loan account PDA controlled SPL token account in which funds will be deposited from (rent exemption check)
    ///    lending pool PDA SPL and from there transferred to borrower's SPL account, owner set to 4 (rent exemption check)
    /// 8. `[]` pda controlled loan account collateral account
    /// 9. `[]` SPL token mint (All spl token should have this account as mint)
    /// 10. `[]` Chainlink feed account (must match feed account pubkey stored in lending pool account)
    /// 11. `[]` Chainlink sol to usd feed account (must match hardcoded key)
    /// 12. `[]` rent sysvar
    /// 13. `[]` Clock sysvar
    /// 14. `[]` token program
    /// 15. `[]` system program
    NewLoan(NewLoan),

    /// Payback loan This works by doing opposite of NewLoan
    /// 0. `[]` Lender spl account
    /// 1. `[signer]` Borrower who wants to payback loan
    /// 2. `[]` SPL token account from which borrower will payback stable coins
    /// 3. `[writable]` Lending pool account which is initialized and owned by this program (PDA account)
    /// 4. `[]` Lending pool PDA controlled SPL token account into which funds will transferred from borrower SPL account
    /// 5. `[writable]` Initialized loan account owned by this program (PDA account)
    /// 6. `[]` Initialized Loan account PDA controlled SPL token account in which funds will be deposited from
    ///    borrower SPL and from there transferred to lending pool SPL account, owner set to 4
    /// 7. `[]` Loan account pda controlled collateral account
    /// 8. `[]` SPL token mint (All spl token should have this account as mint)
    /// 9. `[]` Clock sysvar
    /// 10. `[]` token program
    /// 11. `[]` system program
    PaybackLoan(PaybackLoan),

    /// Backend indicating that this client has defaulted the loan, if yes the SOLs containing in loan
    /// account will go back to lending account
    /// 0. `[]` Lender spl account
    /// 1. `[writable]` Initialized lending account
    /// 2. `[]` Lending account controlled collateral account
    /// 3. `[writable]` Initialized loan account
    /// 4. `[]` Loan account controlled collateral account
    /// 5. `[]` Borrower account
    /// 6. `[]` Clock sysvar
    /// 7. `[]` System program
    DefaultLoan(DefaultLoan),

    /// Closes the lending account and transfers all tokens and SOLs to the lender
    /// 0. `[signer]` Lender who want to close the account
    /// 1. `[]` Lender spl account
    /// 2. `[]` Lender's spl account used to add funds into the lending account
    /// 3. `[writable]` Lending pool account which is initialized and owned by this program (PDA account)
    /// 4. `[]` Lending pool PDA controlled SPL token account into which funds will transferred from borrower SPL account
    /// 5. `[]` Lending pool PDA controlled collateral account
    /// 6. `[]` SPL account in which to deposit the tokens
    /// 7. `[]` SPL token mint
    /// 8. `[]` default account to transfer collateral
    /// 9. `[]` Token program
    /// 10. `[]` system program
    CloseLending(CloseLending),
}

impl LendingPlatformInstructions {
    pub fn unpack(input: &[u8]) -> Result<Self, ProgramError> {
        let (tag, rest) = input.split_first().ok_or(LendingPlatformError::InvalidInstruction)?;

        match tag {
            0 => Ok(Self::InitLendingPoolAccount(InitLendingPoolAccount::unpack(rest)?.0)),
            1 => Ok(Self::NewLendingPool(NewLendingPool::unpack(rest)?.0)),
            2 => Ok(Self::InitLoanAccount(InitLoanAccount::unpack(rest)?.0)),
            3 => Ok(Self::NewLoan(NewLoan::unpack(rest)?.0)),
            4 => Ok(Self::PaybackLoan(PaybackLoan::unpack(rest)?.0)),
            5 => Ok(Self::DefaultLoan(DefaultLoan::unpack(rest)?.0)),
            6 => Ok(Self::CloseLending(CloseLending::unpack(rest)?.0)),
            _ => Err(LendingPlatformError::InvalidInstruction.into()),
        }
    }

    pub fn pack(&self) -> Result<Vec<u8>, ProgramError> {
        match self {
            LendingPlatformInstructions::InitLendingPoolAccount(params) => {
                let mut packed_instruction: Vec<u8> = Vec::with_capacity(params.len() + 1);
                packed_instruction.push(0);
                packed_instruction.extend(params.pack());
                Ok(packed_instruction)
            },
            LendingPlatformInstructions::NewLendingPool(params) => {
                let mut packed_instruction: Vec<u8> = Vec::with_capacity(params.len() + 1);
                packed_instruction.push(1);
                packed_instruction.extend(params.pack());
                Ok(packed_instruction)
            },
            LendingPlatformInstructions::InitLoanAccount(params) => {
                let mut packed_instruction: Vec<u8> = Vec::with_capacity(params.len() + 1);
                packed_instruction.push(2);
                packed_instruction.extend(params.pack());
                Ok(packed_instruction)
            },
            LendingPlatformInstructions::NewLoan(params) => {
                let mut packed_instruction: Vec<u8> = Vec::with_capacity(params.len() + 1);
                packed_instruction.push(3);
                packed_instruction.extend(params.pack());
                Ok(packed_instruction)
            }
            LendingPlatformInstructions::PaybackLoan(params) => {
                let mut packed_instruction: Vec<u8> = Vec::with_capacity(params.len() + 1);
                packed_instruction.push(4);
                packed_instruction.extend(params.pack());
                Ok(packed_instruction)
            },
            LendingPlatformInstructions::DefaultLoan(params) => {
                let mut packed_instruction: Vec<u8> = Vec::with_capacity(params.len() + 1);
                packed_instruction.push(5);
                packed_instruction.extend(params.pack());
                Ok(packed_instruction)
            },
            LendingPlatformInstructions::CloseLending(params) => {
                let mut packed_instruction: Vec<u8> = Vec::with_capacity(params.len() + 1);
                packed_instruction.push(6);
                packed_instruction.extend(params.pack());
                Ok(packed_instruction)
            }
        }
    }
}

pub fn init_lending_pool_account_data(
    program_id: &Pubkey,
    params: InitLendingPoolAccount,
    system_program: &Pubkey,
    lender_account: &Pubkey,
) -> Result<Instruction, ProgramError> {
    let data = LendingPlatformInstructions::InitLendingPoolAccount(params).pack()?;

    let accounts = vec![
        AccountMeta::new(*system_program, false),
        AccountMeta::new(sysvar::rent::id(), false),
        AccountMeta::new(*lender_account, false),
    ];

    Ok(Instruction {
        program_id: *program_id,
        data,
        accounts,
    })
}

pub fn new_lending_pool(
    program_id: &Pubkey,
    params: NewLendingPool,
    lender: &Pubkey,
    coin_spl_account: &Pubkey,
    empty_lending_pool_account: &Pubkey,
    empty_lending_pool_spl_account: &Pubkey,
    chainlink_feed_account: &Pubkey,
    spl_mint_account: &Pubkey,
    token_program: &Pubkey,
) -> Result<Instruction, ProgramError> {
    let data = LendingPlatformInstructions::NewLendingPool(params).pack()?;

    let accounts = vec![
        AccountMeta::new(*lender, true),
        AccountMeta::new(*coin_spl_account, false),
        AccountMeta::new(*empty_lending_pool_account, false),
        AccountMeta::new(*empty_lending_pool_spl_account, false),
        AccountMeta::new(*chainlink_feed_account, false),
        AccountMeta::new(*spl_mint_account, false),
        AccountMeta::new(sysvar::rent::id(), false),
        AccountMeta::new(*token_program, false),
    ];

    Ok(Instruction {
        program_id: *program_id,
        data,
        accounts,
    })
}

pub fn new_loan(
    program_id: &Pubkey,
    params: NewLoan,
    lender_spl_account: &Pubkey,
    borrower: &Pubkey,
    borrower_spl_account: &Pubkey,
    lending_pool_account: &Pubkey,
    lending_pool_spl_account: &Pubkey,
    empty_loan_account: &Pubkey,
    empty_loan_spl_account: &Pubkey,
    loan_collateral_account: &Pubkey,
    spl_token_mint: &Pubkey,
    chainlink_feed_account: &Pubkey,
    chainlink_sol_usd_feed_account: &Pubkey,
    token_program: &Pubkey,
    system_program: &Pubkey,
) -> Result<Instruction, ProgramError> {
    let data = LendingPlatformInstructions::NewLoan(params).pack()?;

    let accounts = vec![
        AccountMeta::new(*lender_spl_account, false),
        AccountMeta::new(*borrower, true),
        AccountMeta::new(*borrower_spl_account, false),
        AccountMeta::new(*lending_pool_account, false),
        AccountMeta::new(*lending_pool_spl_account, false),
        AccountMeta::new(*empty_loan_account, false),
        AccountMeta::new(*empty_loan_spl_account, false),
        AccountMeta::new(*loan_collateral_account, false),
        AccountMeta::new(*spl_token_mint, false),
        AccountMeta::new(*chainlink_feed_account, false),
        AccountMeta::new(*chainlink_sol_usd_feed_account, false),
        AccountMeta::new(sysvar::rent::id(), false),
        AccountMeta::new(sysvar::clock::id(), false),
        AccountMeta::new(*token_program, false),
        AccountMeta::new(*system_program, false)
    ];

    Ok(Instruction {
        program_id: *program_id,
        data,
        accounts,
    })
}

pub fn payback_loan(
    program_id: &Pubkey,
    params: PaybackLoan,
    lender_spl_account: &Pubkey,
    borrower: &Pubkey,
    borrower_spl_token_account: &Pubkey,
    lending_pool_account: &Pubkey,
    lending_pool_spl_account: &Pubkey,
    loan_account: &Pubkey,
    loan_spl_account: &Pubkey,
    loan_collateral_account: &Pubkey,
    spl_token_mint: &Pubkey,
    token_prorgam: &Pubkey,
    system_program: &Pubkey
) -> Result<Instruction, ProgramError> {
    let data = LendingPlatformInstructions::PaybackLoan(params).pack()?;
    let accounts = vec![
        AccountMeta::new(*lender_spl_account, false),
        AccountMeta::new(*borrower, true),
        AccountMeta::new(*borrower_spl_token_account, false),
        AccountMeta::new(*lending_pool_account, false),
        AccountMeta::new(*lending_pool_spl_account, false),
        AccountMeta::new(*loan_account, false),
        AccountMeta::new(*loan_spl_account, false),
        AccountMeta::new(*loan_collateral_account, false),
        AccountMeta::new(*spl_token_mint, false),
        AccountMeta::new(sysvar::clock::id(), false),
        AccountMeta::new(*token_prorgam, false),
        AccountMeta::new(*system_program, false)
    ];

    Ok(Instruction {
        program_id: *program_id,
        data,
        accounts,
    })
}

pub fn default_loan(
    program_id: &Pubkey,
    params: DefaultLoan,
    lender_spl_account: &Pubkey,
    lending_account: &Pubkey,
    lending_pool_collateral_account: &Pubkey,
    loan_account: &Pubkey,
    loan_collateral_account: &Pubkey,
    borrower_account: &Pubkey,
    system_program: &Pubkey
) -> Result<Instruction, ProgramError> {
    let data = LendingPlatformInstructions::DefaultLoan(params).pack()?;

    let accounts = vec![
        AccountMeta::new(*lender_spl_account, false),
        AccountMeta::new(*lending_account, false),
        AccountMeta::new(*lending_pool_collateral_account, false),
        AccountMeta::new(*loan_account, false),
        AccountMeta::new(*loan_collateral_account, false),
        AccountMeta::new(*borrower_account, false),
        AccountMeta::new(sysvar::clock::id(), false),
        AccountMeta::new(*system_program, false)
    ];

    Ok(Instruction {
        program_id: *program_id,
        data,
        accounts,
    })
}

pub fn close_lending(
    program_id: &Pubkey,
    params: CloseLending,
    lender: &Pubkey,
    lender_spl_account: &Pubkey,
    lending_pool_account: &Pubkey,
    lending_pool_spl_account: &Pubkey,
    lending_pool_collateral_account: &Pubkey,
    spl_deposit_account: &Pubkey,
    spl_mint_account: &Pubkey,
    collateral_deposit_account: &Pubkey,
    token_program: &Pubkey,
    system_program: &Pubkey
) -> Result<Instruction, ProgramError> {
    let data = LendingPlatformInstructions::CloseLending(params).pack()?;

    let accounts = vec![
        AccountMeta::new(*lender, true),
        AccountMeta::new(*lender_spl_account, false),
        AccountMeta::new(*lending_pool_account, false),
        AccountMeta::new(*lending_pool_spl_account, false),
        AccountMeta::new(*lending_pool_collateral_account, false),
        AccountMeta::new(*spl_deposit_account, false),
        AccountMeta::new(*spl_mint_account, false),
        AccountMeta::new(*collateral_deposit_account, false),
        AccountMeta::new(*token_program, false),
        AccountMeta::new(*system_program, false)
    ];

    Ok(Instruction {
        program_id: *program_id,
        data,
        accounts,
    })
}
