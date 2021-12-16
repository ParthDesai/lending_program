use crate::error::LendingPlatformError;
use crate::instructions::{LendingPlatformInstructions};
use crate::params::{NewLoan, NewLendingPool, PaybackLoan, DefaultLoan, CloseLending, InitLendingPoolAccount, InitLoanAccount};
use crate::state::{LendingPoolState, LoanState, LENDINGPOOL_OPEN, LOANSTATE_LOANED, LOANSTATE_PAYEDBACK, LOANSTATE_DEFAULTED, LENDINGPOOL_CLOSE};
use solana_program::account_info::{next_account_info, AccountInfo};
use solana_program::entrypoint::ProgramResult;
use solana_program::program::{invoke, invoke_signed};
use solana_program::system_instruction::{create_account, transfer as system_transfer};
use solana_program::program_pack::{IsInitialized, Pack};
use solana_program::pubkey::Pubkey;
use solana_program::rent::Rent;
use solana_program::sysvar::Sysvar;
use solana_program::clock::Clock;
use std::convert::TryInto;
use num_traits::ToPrimitive;
use solana_program::msg;

const CHAINLINK_SOL_USD_FEED_ADDRESS: &str = "FmAmfoyPXiA8Vhhe6MZTr3U6rZfEZ1ctEHay1ysqCqcf";
const SOL_TO_LAMPORT_MULTIPLIER: u128 = 100000000;
pub const SECONDS_PER_YEAR: f64 = 365.242_199 * 24.0 * 60.0 * 60.0;

pub struct Processor;

impl Processor {
    pub fn process(
        program_id: &Pubkey,
        accounts: &[AccountInfo],
        instruction_data: &[u8],
    ) -> ProgramResult {
        let instruction = LendingPlatformInstructions::unpack(instruction_data)?;

        match instruction {
            LendingPlatformInstructions::InitLendingPoolAccount(arg) => Self::process_init_lending_pool(program_id, accounts, arg),
            LendingPlatformInstructions::InitLoanAccount(arg) => Self::process_init_loan(program_id, accounts, arg),
            LendingPlatformInstructions::NewLoan(arg) => Self::new_loan(program_id, accounts, arg),
            LendingPlatformInstructions::NewLendingPool(arg) => Self::new_lending_pool(program_id, accounts, arg),
            LendingPlatformInstructions::PaybackLoan(arg) => Self::payback_loan(program_id, accounts, arg),
            LendingPlatformInstructions::DefaultLoan(arg) => Self::default_loan(program_id, accounts, arg),
            LendingPlatformInstructions::CloseLending(arg) => Self::close_lending(program_id, accounts, arg),
        }
    }

    pub fn process_init_loan(
        program_id: &Pubkey,
        accounts: &[AccountInfo],
        arg: InitLoanAccount
    ) -> ProgramResult {
        let accounts_iter = &mut accounts.iter();

        let system_program = next_account_info(accounts_iter)?;
        let rent_sysvar_account = next_account_info(accounts_iter)?;
        let lending_pool_account = next_account_info(accounts_iter)?;
        let borrower_account = next_account_info(accounts_iter)?;
        let empty_loan_account = next_account_info(accounts_iter)?;

        let rent = Rent::from_account_info(rent_sysvar_account)?;

        if !borrower_account.is_signer {
            return Err(LendingPlatformError::IncorrectSigner.into());
        }

        let borrow_account_bytes = borrower_account.key.to_bytes();
        let lending_pool_account_bytes = lending_pool_account.key.to_bytes();
        let loan_account_pda_signer_seeds: &[&[_]]  = &[
            b"loan_account",  &lending_pool_account_bytes,
            &borrow_account_bytes,
            &[arg.loan_bump_seed]
        ];

        let pda_loan_account = Pubkey::create_program_address(loan_account_pda_signer_seeds, program_id)?;
        if pda_loan_account != *empty_loan_account.key {
            return Err(LendingPlatformError::InvalidAccounts.into());
        }


        let init_loan_account = create_account(
            &borrower_account.key,
            &empty_loan_account.key,
            rent.minimum_balance(LoanState::LEN),
            LoanState::LEN as u64,
            &program_id,
        );

        invoke_signed(
            &init_loan_account,
            &[
                system_program.clone(),
                borrower_account.clone(),
                empty_loan_account.clone(),
            ],
            &[&loan_account_pda_signer_seeds],
        )?;

        Ok(())
    }

    pub fn process_init_lending_pool(
        program_id: &Pubkey,
        accounts: &[AccountInfo],
        arg: InitLendingPoolAccount,
    ) -> ProgramResult {
        let accounts_iter = &mut accounts.iter();

        let system_program = next_account_info(accounts_iter)?;
        let rent_sysvar_account = next_account_info(accounts_iter)?;
        let lender_account = next_account_info(accounts_iter)?;
        let lender_spl_account = next_account_info(accounts_iter)?;
        let empty_lending_pool_account = next_account_info(accounts_iter)?;

        let lender_spl_data = spl_token::state::Account::unpack(&lender_spl_account.data.borrow())?;
        if lender_spl_data.owner != *lender_account.key {
            return Err(LendingPlatformError::InvalidAccounts.into());
        }

        if !lender_account.is_signer {
            return Err(LendingPlatformError::IncorrectSigner.into());
        }

        let rent = Rent::from_account_info(rent_sysvar_account)?;

        let lender_spl_account_bytes = lender_spl_account.key.to_bytes();
        let lending_pool_pda_signer_seeds : &[&[_]] = &[
            b"lending_pool",
            &lender_spl_account_bytes,
            &[arg.bump_seed]
        ];
        let pda_lending_pool = Pubkey::create_program_address(lending_pool_pda_signer_seeds, program_id)?;

        if *empty_lending_pool_account.key != pda_lending_pool {
            return Err(LendingPlatformError::InvalidAccounts.into());
        }

        let init_lending_pool_account = create_account(
            &lender_account.key,
            &empty_lending_pool_account.key,
            rent.minimum_balance(LendingPoolState::LEN),
            LendingPoolState::LEN as u64,
            &program_id,
        );

        invoke_signed(
            &init_lending_pool_account,
            &[
                system_program.clone(),
                lender_account.clone(),
                empty_lending_pool_account.clone(),
            ],
            &[&lending_pool_pda_signer_seeds],
        )?;

        Ok(())
    }

    pub fn new_lending_pool(program_id: &Pubkey, accounts:&[AccountInfo], arg: NewLendingPool) -> ProgramResult {
        let account_info_iter = &mut accounts.iter();
        let lender_account = next_account_info(account_info_iter)?;
        let lender_spl_account = next_account_info(account_info_iter)?;
        let empty_lending_pool_account = next_account_info(account_info_iter)?;
        let empty_lending_pool_spl_account = next_account_info(account_info_iter)?;
        let chainlink_feed_account = next_account_info(account_info_iter)?;
        let spl_mint = next_account_info(account_info_iter)?;
        let rent_sysvar = next_account_info(account_info_iter)?;
        let rent = &Rent::from_account_info(rent_sysvar)?;
        let token_program = next_account_info(account_info_iter)?;

        if !lender_account.is_signer {
            return Err(LendingPlatformError::IncorrectSigner.into());
        }
        let lender_spl_data = spl_token::state::Account::unpack(&lender_spl_account.data.borrow())?;
        if lender_spl_data.owner != *lender_account.key {
            return Err(LendingPlatformError::InvalidAccounts.into());
        }
        if lender_spl_data.mint != *spl_mint.key {
            return Err(LendingPlatformError::InvalidAccounts.into());
        }

        let lender_spl_account_bytes = lender_spl_account.key.to_bytes();
        let lending_pool_pda_signer_seeds : &[&[_]] = &[
            b"lending_pool",
            &lender_spl_account_bytes,
            &[arg.bump_seed]
        ];
        let pda_lending_pool = Pubkey::create_program_address(lending_pool_pda_signer_seeds, program_id)?;

        if *empty_lending_pool_account.key != pda_lending_pool {
            return Err(LendingPlatformError::InvalidAccounts.into());
        }

        if !rent.is_exempt(empty_lending_pool_account.lamports(), empty_lending_pool_account.data_len()) {
            msg!("Lending pool account is not rent exempt");
            return Err(LendingPlatformError::NotRentExempt.into());
        }

        if !rent.is_exempt(empty_lending_pool_spl_account.lamports(), empty_lending_pool_spl_account.data_len()) {
            msg!("Lending pool spl account is not rent exempt");
            return Err(LendingPlatformError::NotRentExempt.into());
        }

        let mut uninit_lending_pool_state = LendingPoolState::unpack_unchecked(&empty_lending_pool_account.data.borrow())?;
        if uninit_lending_pool_state.is_initialized() {
            return Err(LendingPlatformError::AlreadyInitialized.into());
        }

        // Checking if the feed is valid or not
        let feed_result = chainlink::get_price(&chainlink::id(), chainlink_feed_account)?;
        if feed_result.is_none() {
            return Err(LendingPlatformError::InvalidAccounts.into());
        }

        let initialize_account = spl_token::instruction::initialize_account(
            token_program.key,
            empty_lending_pool_spl_account.key,
            spl_mint.key,
            empty_lending_pool_account.key,
        )?;
        invoke(
            &initialize_account,
            &[
                token_program.clone(),
                empty_lending_pool_spl_account.clone(),
                spl_mint.clone(),
                empty_lending_pool_account.clone(),
                rent_sysvar.clone(),
            ],
        )?;

        let transfer_coins = spl_token::instruction::transfer(
            token_program.key,
            lender_spl_account.key,
            empty_lending_pool_spl_account.key,
            lender_account.key,
            &[],
            arg.total_lending_amount
        )?;

        invoke(
            &transfer_coins,
            &[
                token_program.clone(),
                lender_spl_account.clone(),
                empty_lending_pool_spl_account.clone(),
                lender_account.clone(),
            ],
        )?;


        uninit_lending_pool_state.number_of_outstanding_loans = 0;
        uninit_lending_pool_state.lender = *lender_account.key;
        uninit_lending_pool_state.chainlink_feed_account = *chainlink_feed_account.key;
        uninit_lending_pool_state.max_payback_time = arg.max_payback_time;
        uninit_lending_pool_state.expected_apy = arg.expected_apy;
        uninit_lending_pool_state.status = LENDINGPOOL_OPEN;
        uninit_lending_pool_state.initialized = true;
        uninit_lending_pool_state.coin_amount = arg.total_lending_amount as u128;
        uninit_lending_pool_state.collateral_amount = 0;


        LendingPoolState::pack(uninit_lending_pool_state, &mut empty_lending_pool_account.data.borrow_mut())?;

        Ok(())
    }

    pub fn new_loan(program_id: &Pubkey, accounts: &[AccountInfo], arg: NewLoan) -> ProgramResult {
        let account_info_iter = &mut accounts.iter();
        let lender_spl_account = next_account_info(account_info_iter)?;
        let borrower_account = next_account_info(account_info_iter)?;
        let borrower_spl_account = next_account_info(account_info_iter)?;
        let lending_pool_account = next_account_info(account_info_iter)?;
        let lending_pool_spl_account = next_account_info(account_info_iter)?;
        let empty_loan_account = next_account_info(account_info_iter)?;
        let loan_collateral_account = next_account_info(account_info_iter)?;
        let spl_mint = next_account_info(account_info_iter)?;
        let chainlink_feed_account = next_account_info(account_info_iter)?;
        let chainlink_sol_usd_feed_account = next_account_info(account_info_iter)?;
        let rent_sysvar = next_account_info(account_info_iter)?;
        let rent = &Rent::from_account_info(rent_sysvar)?;
        let clock_sysvar = next_account_info(account_info_iter)?;
        let clock = &Clock::from_account_info(clock_sysvar)?;
        let token_program = next_account_info(account_info_iter)?;
        let system_program = next_account_info(account_info_iter)?;

        if !borrower_account.is_signer {
            return Err(LendingPlatformError::IncorrectSigner.into());
        }
        let borrower_spl_data = spl_token::state::Account::unpack(&borrower_spl_account.data.borrow())?;
        if borrower_spl_data.owner != *borrower_account.key {
            return Err(LendingPlatformError::InvalidAccounts.into());
        }
        if borrower_spl_data.mint != *spl_mint.key {
            return Err(LendingPlatformError::InvalidAccounts.into());
        }

        let lender_spl_account_bytes = lender_spl_account.key.to_bytes();
        let lending_pool_pda_signer_seeds : &[&[_]] = &[
            b"lending_pool",
            &lender_spl_account_bytes,
            &[arg.lending_pool_bump_seed]
        ];
        let lending_pool_pda = Pubkey::create_program_address(lending_pool_pda_signer_seeds, program_id)?;
        if lending_pool_pda != *lending_pool_account.key {
            return Err(LendingPlatformError::InvalidAccounts.into());
        }

        let mut lending_pool_state = LendingPoolState::unpack(&lending_pool_account.data.borrow())?;
        let lending_pool_spl_data = spl_token::state::Account::unpack(&lending_pool_spl_account.data.borrow())?;
        if lending_pool_spl_data.owner != *lending_pool_account.key {
            return Err(LendingPlatformError::InvalidAccounts.into());
        }

        if lending_pool_state.status != LENDINGPOOL_OPEN {
            return Err(LendingPlatformError::LendingPoolNotOpen.into());
        }

        if lending_pool_state.coin_amount < arg.amount as u128 {
            return Err(LendingPlatformError::NotEnoughFundsInLendingPool.into());
        }

        // Lending account is associated with lender
        // loan account is associated with lending account and borrower

        let borrow_account_bytes = borrower_account.key.to_bytes();
        let lending_pool_account_bytes = lending_pool_account.key.to_bytes();
        let loan_account_pda_signer_seeds: &[&[_]]  = &[
            b"loan_account",  &lending_pool_account_bytes,
            &borrow_account_bytes,
            &[arg.loan_bump_seed]
        ];
        let pda_loan_account = Pubkey::create_program_address(loan_account_pda_signer_seeds, program_id)?;
        if pda_loan_account != *empty_loan_account.key {
            return Err(LendingPlatformError::InvalidAccounts.into());
        }

        let loan_account_collateral_pda_signer_seeds: &[&[_]]  = &[
            b"loan_account_collateral",  &lending_pool_account_bytes,
            &borrow_account_bytes,
            &[arg.loan_collateral_bump_seed]
        ];
        let pda_loan_account_collateral = Pubkey::create_program_address(loan_account_collateral_pda_signer_seeds, program_id)?;
        if pda_loan_account_collateral != *loan_collateral_account.key {
            return Err(LendingPlatformError::InvalidAccounts.into());
        }

        if !rent.is_exempt(empty_loan_account.lamports(), empty_loan_account.data_len()) {
            return Err(LendingPlatformError::NotRentExempt.into());
        }

        let mut uninit_loan_state = LoanState::unpack_unchecked(&empty_loan_account.data.borrow())?;
        if uninit_loan_state.is_initialized() {
            return Err(LendingPlatformError::AlreadyInitialized.into());
        }

        if chainlink_sol_usd_feed_account.key.to_string() != CHAINLINK_SOL_USD_FEED_ADDRESS {
            return Err(LendingPlatformError::InvalidSOLUSDFeed.into());
        }

        if *chainlink_feed_account.key != lending_pool_state.chainlink_feed_account {
            return Err(LendingPlatformError::InvalidAccounts.into());
        }

        let sol_to_usd_conversion_rate = chainlink::get_price(&chainlink::id(), chainlink_sol_usd_feed_account)?;
        if sol_to_usd_conversion_rate.is_none() {
            return Err(LendingPlatformError::InvalidAccounts.into());
        }
        let sol_to_usd_conversion_rate = sol_to_usd_conversion_rate.unwrap();

        let stablecoin_to_usd_conversion_rate = chainlink::get_price(&chainlink::id(), chainlink_feed_account)?;
        if stablecoin_to_usd_conversion_rate.is_none() {
            return Err(LendingPlatformError::InvalidAccounts.into());
        }
        let stablecoin_to_usd_conversion_rate = stablecoin_to_usd_conversion_rate.unwrap();

        let target_usd = arg.amount as u128 * stablecoin_to_usd_conversion_rate;
        let collateral_usd = (100 * target_usd) / 60;

        msg!("Stable coin to usd conversion rate is: {}, target_usd: {}, collateral_usd: {}, sol_to_usd_conversion_rate: {}, collateral_sols: {}", stablecoin_to_usd_conversion_rate, target_usd, collateral_usd, sol_to_usd_conversion_rate, collateral_usd / sol_to_usd_conversion_rate);

        // This much sols need to be taken from the borrower account
        let collateral_sols = (collateral_usd / sol_to_usd_conversion_rate) + 1;
        let collateral_lamports = collateral_sols.checked_mul(SOL_TO_LAMPORT_MULTIPLIER);
        if collateral_lamports.is_none() {
            return Err(LendingPlatformError::InvalidAccounts.into());
        }

        let collateral_lamports = collateral_lamports.unwrap();
        let transfer_amount = collateral_lamports.try_into().map_err(|_e| LendingPlatformError::InvalidAccounts)?;
        let collateral_transfer_instruction = system_transfer(borrower_account.key, loan_collateral_account.key, transfer_amount);
        invoke(
            &collateral_transfer_instruction,
            &[
                system_program.clone(),
                borrower_account.clone(),
                loan_collateral_account.clone()
            ]
        )?;

        let stablecoin_transfer_instruction = spl_token::instruction::transfer(
            token_program.key,
            lending_pool_spl_account.key,
            borrower_spl_account.key,
            lending_pool_account.key,
            &[],
            arg.amount
        )?;

        invoke_signed(
            &stablecoin_transfer_instruction,
            &[
                token_program.clone(),
                lending_pool_spl_account.clone(),
                borrower_spl_account.clone(),
                lending_pool_account.clone()
            ],
            &[&lending_pool_pda_signer_seeds]
        )?;

        uninit_loan_state.initialized = true;

        uninit_loan_state.amount = arg.amount;
        uninit_loan_state.validity_till = lending_pool_state.max_payback_time;
        uninit_loan_state.borrowed_on = clock.unix_timestamp as u64;
        uninit_loan_state.lending_account = *lending_pool_account.key;
        uninit_loan_state.borrower = *borrower_account.key;
        uninit_loan_state.status = LOANSTATE_LOANED;
        uninit_loan_state.expected_apy = lending_pool_state.expected_apy;
        uninit_loan_state.collateral_lamports = collateral_lamports;

        LoanState::pack(uninit_loan_state, &mut empty_loan_account.data.borrow_mut())?;

        lending_pool_state.coin_amount -= arg.amount as u128;
        lending_pool_state.number_of_outstanding_loans += 1;
        LendingPoolState::pack(lending_pool_state, &mut lending_pool_account.data.borrow_mut())?;

        Ok(())
    }

    pub fn payback_loan(program_id: &Pubkey, accounts: &[AccountInfo], arg: PaybackLoan) -> ProgramResult {
        let account_info_iter = &mut accounts.iter();
        let lender_spl_account = next_account_info(account_info_iter)?;
        let borrower_account = next_account_info(account_info_iter)?;
        let borrower_spl_account = next_account_info(account_info_iter)?;
        let lending_pool_account = next_account_info(account_info_iter)?;
        let lending_pool_spl_account = next_account_info(account_info_iter)?;
        let loan_account = next_account_info(account_info_iter)?;
        let loan_collateral_account = next_account_info(account_info_iter)?;
        let spl_mint = next_account_info(account_info_iter)?;
        let clock_sysvar = next_account_info(account_info_iter)?;
        let clock = &Clock::from_account_info(clock_sysvar)?;
        let token_program = next_account_info(account_info_iter)?;
        let system_program = next_account_info(account_info_iter)?;

        if !borrower_account.is_signer {
            return Err(LendingPlatformError::IncorrectSigner.into());
        }
        let borrower_spl_data = spl_token::state::Account::unpack(&borrower_spl_account.data.borrow())?;
        if borrower_spl_data.owner != *borrower_account.key {
            return Err(LendingPlatformError::InvalidAccounts.into());
        }
        if borrower_spl_data.mint != *spl_mint.key {
            return Err(LendingPlatformError::InvalidAccounts.into());
        }

        let mut lending_pool_state = LendingPoolState::unpack(&lending_pool_account.data.borrow())?;
        if lending_pool_state.status != LENDINGPOOL_OPEN {
            return Err(LendingPlatformError::LendingPoolNotOpen.into());
        }

        let lending_pool_spl_data = spl_token::state::Account::unpack(&lending_pool_spl_account.data.borrow())?;
        if lending_pool_spl_data.owner != *lending_pool_account.key {
            return Err(LendingPlatformError::InvalidAccounts.into());
        }
        if lending_pool_spl_data.mint != *spl_mint.key {
            return Err(LendingPlatformError::InvalidAccounts.into());
        }

        let lender_spl_account_bytes = lender_spl_account.key.to_bytes();
        let lending_pool_pda_signer_seeds : &[&[_]] = &[
            b"lending_pool",
            &lender_spl_account_bytes,
            &[arg.lending_pool_bump_seed]
        ];
        let lending_pool_pda = Pubkey::create_program_address(lending_pool_pda_signer_seeds, program_id)?;
        if lending_pool_pda != *lending_pool_account.key {
            return Err(LendingPlatformError::InvalidAccounts.into());
        }
        if lending_pool_account.owner != program_id {
            return Err(LendingPlatformError::InvalidAccounts.into());
        }

        // Lending account is associated with lender
        // loan account is associated with lending account and borrower
        let borrow_account_bytes = borrower_account.key.to_bytes();
        let lending_pool_account_bytes = lending_pool_account.key.to_bytes();
        let loan_account_pda_signer_seeds: &[&[_]]  = &[
            b"loan_account",  &lending_pool_account_bytes,
            &borrow_account_bytes,
            &[arg.loan_bump_seed]
        ];
        let pda_loan_account = Pubkey::create_program_address(loan_account_pda_signer_seeds, program_id)?;
        if pda_loan_account != *loan_account.key {
            return Err(LendingPlatformError::InvalidAccounts.into());
        }
        if loan_account.owner != program_id {
            return Err(LendingPlatformError::InvalidAccounts.into());
        }

        let loan_account_collateral_pda_signer_seeds: &[&[_]]  = &[
            b"loan_account_collateral",  &lending_pool_account_bytes,
            &borrow_account_bytes,
            &[arg.loan_collateral_bump_seed]
        ];
        let pda_loan_account_collateral = Pubkey::create_program_address(loan_account_collateral_pda_signer_seeds, program_id)?;
        if pda_loan_account_collateral != *loan_collateral_account.key {
            return Err(LendingPlatformError::InvalidAccounts.into());
        }

        let mut loan_state = LoanState::unpack(&loan_account.data.borrow())?;
        if loan_state.lending_account != *lending_pool_account.key {
            return Err(LendingPlatformError::InvalidAccounts.into());
        }


        let borrow_time = clock.unix_timestamp as u64 - loan_state.borrowed_on;
        if borrow_time > loan_state.validity_till {
            return Err(LendingPlatformError::InvalidAccounts.into());
        }

        let interest_in_a_year = (loan_state.amount * loan_state.expected_apy as u64) / 100;
        let interest_due = borrow_time.checked_mul(interest_in_a_year).and_then(|intermediate_mul| {
                msg!("floating interest is: {} (Will be rounded up to next int)", (intermediate_mul as f64 / SECONDS_PER_YEAR));
                (intermediate_mul as f64 / SECONDS_PER_YEAR).ceil().to_u64()
            }
        );
        if interest_due.is_none() {
            return Err(LendingPlatformError::InvalidAccounts.into());
        }
        let interest_due = interest_due.unwrap() as u64;
        let principle_amount = loan_state.amount;

        let principle_transfer_instruction = spl_token::instruction::transfer(
            token_program.key,
            borrower_spl_account.key,
            lending_pool_spl_account.key,
            borrower_account.key,
            &[],
            principle_amount
        )?;

        invoke(
            &principle_transfer_instruction,
            &[
                token_program.clone(),
                borrower_spl_account.clone(),
                lending_pool_spl_account.clone(),
                borrower_account.clone()
            ]
        )?;

        let interest_transfer_instruction = spl_token::instruction::transfer(
            token_program.key,
            borrower_spl_account.key,
            lender_spl_account.key,
            borrower_account.key,
            &[],
            interest_due
        )?;

        invoke(
            &interest_transfer_instruction,
            &[
                token_program.clone(),
                borrower_spl_account.clone(),
                lender_spl_account.clone(),
                borrower_account.clone()
            ]
        )?;


        let transfer_amount = loan_state.collateral_lamports.try_into().map_err(|_e| LendingPlatformError::InvalidAccounts)?;
        let collateral_transfer_instruction = system_transfer(loan_collateral_account.key, borrower_account.key, transfer_amount);
        invoke_signed(
            &collateral_transfer_instruction,
            &[
                system_program.clone(),
                loan_collateral_account.clone(),
                borrower_account.clone()
            ],
            &[&loan_account_collateral_pda_signer_seeds]
        )?;


        loan_state.status = LOANSTATE_PAYEDBACK;
        LoanState::pack(loan_state, &mut loan_account.data.borrow_mut())?;

        lending_pool_state.coin_amount += principle_amount as u128;
        lending_pool_state.number_of_outstanding_loans -= 1;
        LendingPoolState::pack(lending_pool_state, &mut lending_pool_account.data.borrow_mut())?;

        Ok(())

    }

    pub fn default_loan(program_id: &Pubkey, accounts: &[AccountInfo], arg: DefaultLoan) -> ProgramResult {
        let account_info_iter = &mut accounts.iter();
        let lender_spl_account = next_account_info(account_info_iter)?;
        let lending_pool_account = next_account_info(account_info_iter)?;
        let lending_pool_collateral_account = next_account_info(account_info_iter)?;
        let loan_account = next_account_info(account_info_iter)?;
        let loan_collateral_account = next_account_info(account_info_iter)?;
        let borrower_account = next_account_info(account_info_iter)?;
        let clock_sysvar = next_account_info(account_info_iter)?;
        let system_program = next_account_info(account_info_iter)?;

        let clock = &Clock::from_account_info(&clock_sysvar)?;

        let mut lending_pool_state = LendingPoolState::unpack(&lending_pool_account.data.borrow())?;
        if lending_pool_state.status != LENDINGPOOL_OPEN {
            return Err(LendingPlatformError::LendingPoolNotOpen.into());
        }

        let mut loan_state = LoanState::unpack(&loan_account.data.borrow())?;

        if loan_state.lending_account != *lending_pool_account.key {
            return Err(LendingPlatformError::InvalidAccounts.into());
        }

        let lender_spl_account_bytes = lender_spl_account.key.to_bytes();
        let lending_pool_pda_signer_seeds : &[&[_]] = &[
            b"lending_pool",
            &lender_spl_account_bytes,
            &[arg.lending_pool_bump_seed]
        ];
        let lending_pool_pda = Pubkey::create_program_address(lending_pool_pda_signer_seeds, program_id)?;
        if lending_pool_pda != *lending_pool_account.key {
            return Err(LendingPlatformError::InvalidAccounts.into());
        }
        if lending_pool_account.owner != program_id {
            return Err(LendingPlatformError::InvalidAccounts.into());
        }

        // Lending account is associated with lender
        // loan account is associated with lending account and borrower
        let borrow_account_bytes = borrower_account.key.to_bytes();
        let lending_pool_account_bytes = lending_pool_account.key.to_bytes();
        let loan_account_pda_signer_seeds: &[&[_]]  = &[
            b"loan_account",  &lending_pool_account_bytes,
            &borrow_account_bytes,
            &[arg.loan_bump_seed]
        ];
        let pda_loan_account = Pubkey::create_program_address(loan_account_pda_signer_seeds, program_id)?;
        if pda_loan_account != *loan_account.key {
            return Err(LendingPlatformError::InvalidAccounts.into());
        }
        if loan_account.owner != program_id {
            return Err(LendingPlatformError::InvalidAccounts.into());
        }

        let loan_account_collateral_pda_signer_seeds: &[&[_]]  = &[
            b"loan_account_collateral",  &lending_pool_account_bytes,
            &borrow_account_bytes,
            &[arg.loan_collateral_seed]
        ];
        let pda_loan_account_collateral = Pubkey::create_program_address(loan_account_collateral_pda_signer_seeds, program_id)?;
        if pda_loan_account_collateral != *loan_collateral_account.key {
            return Err(LendingPlatformError::InvalidAccounts.into());
        }

        let current_time = clock.unix_timestamp;
        if current_time < 0 {
            return Err(LendingPlatformError::InvalidAccounts.into());
        }
        if (loan_state.borrowed_on + loan_state.validity_till) > current_time as u64 {
            return Err(LendingPlatformError::StillValidLoan.into());
        }

        let transfer_amount = loan_state.collateral_lamports.try_into().map_err(|_e| LendingPlatformError::InvalidAccounts)?;
        let collateral_transfer_instruction = system_transfer(loan_collateral_account.key, lending_pool_collateral_account.key,  transfer_amount);
        invoke_signed(
            &collateral_transfer_instruction,
            &[
                system_program.clone(),
                loan_collateral_account.clone(),
                lending_pool_collateral_account.clone()
            ],
            &[&loan_account_collateral_pda_signer_seeds]
        )?;

        lending_pool_state.collateral_amount += loan_state.collateral_lamports;
        lending_pool_state.number_of_outstanding_loans -= 1;
        LendingPoolState::pack(lending_pool_state, &mut lending_pool_account.data.borrow_mut())?;

        loan_state.status = LOANSTATE_DEFAULTED;
        LoanState::pack(loan_state, &mut loan_account.data.borrow_mut())?;

        Ok(())
    }

    pub fn close_lending(program_id: &Pubkey, accounts: &[AccountInfo], arg: CloseLending) -> ProgramResult {
        let account_info_iter = &mut accounts.iter();
        let lender_account = next_account_info(account_info_iter)?;
        let lender_spl_account = next_account_info(account_info_iter)?;
        let lending_pool_account = next_account_info(account_info_iter)?;
        let lending_pool_spl_account = next_account_info(account_info_iter)?;
        let lending_pool_collateral_account = next_account_info(account_info_iter)?;
        let spl_deposit_account = next_account_info(account_info_iter)?;
        let spl_mint_account = next_account_info(account_info_iter)?;
        let collateral_deposit_account = next_account_info(account_info_iter)?;
        let token_program = next_account_info(account_info_iter)?;
        let system_program = next_account_info(account_info_iter)?;

        if !lender_account.is_signer {
            return Err(LendingPlatformError::IncorrectSigner.into());
        }

        let lender_spl_account_bytes = lender_spl_account.key.to_bytes();
        let lending_pool_pda_signer_seeds : &[&[_]] = &[
            b"lending_pool",
            &lender_spl_account_bytes,
            &[arg.lending_pool_bump_seed]
        ];
        let pda_lending_pool = Pubkey::create_program_address(lending_pool_pda_signer_seeds, program_id)?;

        if *lending_pool_account.key != pda_lending_pool {
            return Err(LendingPlatformError::InvalidAccounts.into());
        }

        if lending_pool_account.owner != program_id {
            return Err(LendingPlatformError::InvalidAccounts.into());
        }

        let mut lending_pool_state = LendingPoolState::unpack(&lending_pool_account.data.borrow())?;
        if lending_pool_state.lender != *lender_account.key {
            return Err(LendingPlatformError::InvalidAccounts.into());
        }
        if lending_pool_state.number_of_outstanding_loans > 0 {
            return Err(LendingPlatformError::LendingPoolHasOutstandingLoan.into());
        }

        let lending_pool_spl_data = spl_token::state::Account::unpack(&lending_pool_spl_account.data.borrow())?;
        if lending_pool_spl_data.owner != *lending_pool_account.key {
            return Err(LendingPlatformError::InvalidAccounts.into());
        }
        if lending_pool_spl_data.mint != *spl_mint_account.key {
            return Err(LendingPlatformError::InvalidAccounts.into());
        }

        let deposit_account_spl_data = spl_token::state::Account::unpack(&spl_deposit_account.data.borrow())?;
        if deposit_account_spl_data.mint != *spl_mint_account.key {
            return Err(LendingPlatformError::InvalidAccounts.into());
        }

        let lender_spl_account_bytes = lender_spl_account.key.to_bytes();
        let lending_pool_collateral_pda_signer_seeds : &[&[_]] = &[
            b"lending_pool_collateral",
            &lender_spl_account_bytes,
            &[arg.lending_pool_collateral_seed]
        ];
        let pda_lending_pool_collateral = Pubkey::create_program_address(lending_pool_collateral_pda_signer_seeds, program_id)?;

        if *lending_pool_collateral_account.key != pda_lending_pool_collateral {
            return Err(LendingPlatformError::InvalidAccounts.into());
        }

        let transfer_amount = lending_pool_state.collateral_amount.try_into().map_err(|_e| LendingPlatformError::InvalidAccounts)?;
        let collateral_transfer_instruction = system_transfer(lending_pool_collateral_account.key, collateral_deposit_account.key, transfer_amount);
        invoke_signed(
            &collateral_transfer_instruction,
            &[
                system_program.clone(),
                lending_pool_collateral_account.clone(),
                collateral_deposit_account.clone()
            ],
            &[&lending_pool_collateral_pda_signer_seeds]
        )?;

        let stablecoin_transfer_instruction = spl_token::instruction::transfer(
            token_program.key,
            lending_pool_spl_account.key,
            spl_deposit_account.key,
            lending_pool_account.key,
            &[],
            lending_pool_state.coin_amount as u64
        )?;
        invoke_signed(
            &stablecoin_transfer_instruction,
            &[
                token_program.clone(),
                lending_pool_spl_account.clone(),
                spl_deposit_account.clone(),
                lending_pool_account.clone()
            ],
            &[&lending_pool_pda_signer_seeds]
        )?;

        lending_pool_state.status = LENDINGPOOL_CLOSE;
        LendingPoolState::pack(lending_pool_state, &mut lending_pool_account.data.borrow_mut())?;
        Ok(())
    }
}
