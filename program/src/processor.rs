use crate::error::LendingPlatformError;
use crate::instructions::{LendingPlatformInstructions};
use crate::params::{NewLoan, NewLendingPool, PaybackLoan, DefaultLoan, CloseLending};
use crate::state::{LendingPoolState, LoanState, LENDINGPOOL_OPEN, LOANSTATE_LOANED, LOANSTATE_PAYEDBACK, LOANSTATE_DEFAULTED, LENDINGPOOL_CLOSE};
use solana_program::account_info::{next_account_info, AccountInfo};
use solana_program::entrypoint::ProgramResult;
use solana_program::program::{invoke, invoke_signed};
use solana_program::system_instruction::{transfer as system_transfer};
use solana_program::program_pack::{IsInitialized, Pack};
use solana_program::pubkey::Pubkey;
use solana_program::rent::Rent;
use solana_program::sysvar::Sysvar;
use solana_program::clock::Clock;
use std::convert::TryInto;

const CHAINLINK_SOL_USD_FEED_ADDRESS: &str = "FmAmfoyPXiA8Vhhe6MZTr3U6rZfEZ1ctEHay1ysqCqcf";
const SOL_TO_LAMPORT_MULTIPLIER: u128 = 100000000;
const YEAR_IN_SECONDS: u64 = 365 * 24 * 60 * 60;

pub struct Processor;

impl Processor {
    pub fn process(
        program_id: &Pubkey,
        accounts: &[AccountInfo],
        instruction_data: &[u8],
    ) -> ProgramResult {
        let instruction = LendingPlatformInstructions::unpack(instruction_data)?;

        match instruction {
            LendingPlatformInstructions::NewLoan(arg) => Self::new_loan(program_id, accounts, arg),
            LendingPlatformInstructions::NewLendingPool(arg) => Self::new_lending_pool(program_id, accounts, arg),
            LendingPlatformInstructions::PaybackLoan(arg) => Self::payback_loan(program_id, accounts, arg),
            LendingPlatformInstructions::DefaultLoan(arg) => Self::default_loan(program_id, accounts, arg),
            LendingPlatformInstructions::CloseLending(arg) => Self::close_lending(program_id, accounts, arg)
        }
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

        if !rent.is_exempt(empty_lending_pool_account.lamports(), empty_lending_pool_account.data_len()) {
            return Err(LendingPlatformError::InvalidAccounts.into());
        }

        let lender_account_bytes = lender_account.key.to_bytes();
        let lending_pool_pda_signer_seeds : &[&[_]] = &[
            b"lending_pool",
            &lender_account_bytes,
            &[arg.bump_seed]
        ];
        let pda_lending_pool = Pubkey::create_program_address(lending_pool_pda_signer_seeds, program_id)?;

        if *empty_lending_pool_account.key != pda_lending_pool {
            return Err(LendingPlatformError::InvalidAccounts.into());
        }

        if !rent.is_exempt(empty_lending_pool_spl_account.lamports(), empty_lending_pool_spl_account.data_len()) {
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
        let borrower_account = next_account_info(account_info_iter)?;
        let borrower_spl_account = next_account_info(account_info_iter)?;
        let lending_pool_account = next_account_info(account_info_iter)?;
        let lending_pool_spl_account = next_account_info(account_info_iter)?;
        let lender_account = next_account_info(account_info_iter)?;
        let empty_loan_account = next_account_info(account_info_iter)?;
        let empty_loan_spl_account = next_account_info(account_info_iter)?;
        let spl_mint = next_account_info(account_info_iter)?;
        let chainlink_feed_account = next_account_info(account_info_iter)?;
        let chainlink_sol_usd_feed_account = next_account_info(account_info_iter)?;
        let rent_sysvar = next_account_info(account_info_iter)?;
        let rent = &Rent::from_account_info(rent_sysvar)?;
        let clock_sysvar = next_account_info(account_info_iter)?;
        let clock = &Clock::from_account_info(clock_sysvar)?;
        let token_program = next_account_info(account_info_iter)?;

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

        let lender_account_bytes = lender_account.key.to_bytes();
        let lending_pool_pda_signer_seeds : &[&[_]] = &[
            b"lending_pool",
            &lender_account_bytes,
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
            return Err(LendingPlatformError::InvalidAccounts.into());
        }

        if lending_pool_state.coin_amount < arg.amount as u128 {
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
        if pda_loan_account != *empty_loan_account.key {
            return Err(LendingPlatformError::InvalidAccounts.into());
        }

        if !rent.is_exempt(empty_loan_account.lamports(), empty_loan_account.data_len()) {
            return Err(LendingPlatformError::NotRentExempt.into());
        }

        let mut uninit_loan_state = LoanState::unpack_unchecked(&empty_loan_account.data.borrow())?;
        if uninit_loan_state.is_initialized() {
            return Err(LendingPlatformError::AlreadyInitialized.into());
        }

        if !rent.is_exempt(empty_loan_spl_account.lamports(), empty_loan_spl_account.data_len()) {
            return Err(LendingPlatformError::NotRentExempt.into());
        }

        if chainlink_sol_usd_feed_account.key.to_string() != CHAINLINK_SOL_USD_FEED_ADDRESS {
            return Err(LendingPlatformError::InvalidAccounts.into());
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

        // This much sols need to be taken from the borrower account
        let collateral_sols = (collateral_usd / sol_to_usd_conversion_rate) + 1;
        let collateral_lamports = collateral_sols.checked_mul(SOL_TO_LAMPORT_MULTIPLIER);
        if collateral_lamports.is_none() {
            return Err(LendingPlatformError::InvalidAccounts.into());
        }

        let collateral_lamports = collateral_lamports.unwrap();
        let transfer_amount = collateral_lamports.try_into().map_err(|_e| LendingPlatformError::InvalidAccounts)?;
        let collateral_transfer_instruction = system_transfer(borrower_account.key, empty_loan_account.key, transfer_amount);
        invoke(
            &collateral_transfer_instruction,
            &[
                borrower_account.clone(),
                empty_loan_account.clone()
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
        let borrower_account = next_account_info(account_info_iter)?;
        let borrower_spl_account = next_account_info(account_info_iter)?;
        let lending_pool_account = next_account_info(account_info_iter)?;
        let lending_pool_spl_account = next_account_info(account_info_iter)?;
        let loan_account = next_account_info(account_info_iter)?;
        let loan_spl_account = next_account_info(account_info_iter)?;
        let spl_mint = next_account_info(account_info_iter)?;
        let clock_sysvar = next_account_info(account_info_iter)?;
        let clock = &Clock::from_account_info(clock_sysvar)?;
        let token_program = next_account_info(account_info_iter)?;

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
            return Err(LendingPlatformError::InvalidAccounts.into());
        }

        let lending_pool_spl_data = spl_token::state::Account::unpack(&lending_pool_spl_account.data.borrow())?;
        if lending_pool_spl_data.owner != *lending_pool_account.key {
            return Err(LendingPlatformError::InvalidAccounts.into());
        }
        if lending_pool_spl_data.mint != *spl_mint.key {
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

        let mut loan_state = LoanState::unpack(&loan_account.data.borrow())?;
        let loan_spl_data = spl_token::state::Account::unpack(&loan_spl_account.data.borrow())?;
        if loan_spl_data.owner != *loan_account.key {
            return Err(LendingPlatformError::InvalidAccounts.into());
        }
        if loan_spl_data.mint != *spl_mint.key {
            return Err(LendingPlatformError::InvalidAccounts.into());
        }
        if loan_state.lending_account != *lending_pool_account.key {
            return Err(LendingPlatformError::InvalidAccounts.into());
        }


        let borrow_time = clock.unix_timestamp as u64 - loan_state.borrowed_on;
        if borrow_time > loan_state.validity_till {
            return Err(LendingPlatformError::InvalidAccounts.into());
        }

        let interest_in_a_year = (loan_state.amount * loan_state.expected_apy as u64) / 100;
        let interest_due = borrow_time.checked_mul(interest_in_a_year).and_then(|intermediate_mul| (intermediate_mul as u128).checked_div(YEAR_IN_SECONDS as u128));
        if interest_due.is_none() {
            return Err(LendingPlatformError::InvalidAccounts.into());
        }
        let interest_due = interest_due.unwrap() as u64;
        let total_amount = interest_due + loan_state.amount;

        let stablecoin_transfer_instruction = spl_token::instruction::transfer(
            token_program.key,
            borrower_spl_account.key,
            lending_pool_spl_account.key,
            borrower_account.key,
            &[],
            total_amount
        )?;

        invoke(
            &stablecoin_transfer_instruction,
            &[
                token_program.clone(),
                borrower_spl_account.clone(),
                lending_pool_spl_account.clone(),
                borrower_account.clone()
            ]
        )?;


        let transfer_amount = loan_state.collateral_lamports.try_into().map_err(|_e| LendingPlatformError::InvalidAccounts)?;
        let collateral_transfer_instruction = system_transfer(loan_account.key, borrower_account.key, transfer_amount);
        invoke_signed(
            &collateral_transfer_instruction,
            &[
                loan_account.clone(),
                borrower_account.clone()
            ],
            &[&loan_account_pda_signer_seeds]
        )?;


        loan_state.status = LOANSTATE_PAYEDBACK;
        LoanState::pack(loan_state, &mut loan_account.data.borrow_mut())?;

        lending_pool_state.coin_amount += total_amount as u128;
        lending_pool_state.number_of_outstanding_loans -= 1;
        LendingPoolState::pack(lending_pool_state, &mut lending_pool_account.data.borrow_mut())?;

        Ok(())

    }

    pub fn default_loan(program_id: &Pubkey, accounts: &[AccountInfo], arg: DefaultLoan) -> ProgramResult {
        let account_info_iter = &mut accounts.iter();
        let lending_pool_account = next_account_info(account_info_iter)?;
        let loan_account = next_account_info(account_info_iter)?;
        let borrower_account = next_account_info(account_info_iter)?;
        let clock_sysvar = next_account_info(account_info_iter)?;
        let clock = &Clock::from_account_info(&clock_sysvar)?;

        let mut lending_pool_state = LendingPoolState::unpack(&lending_pool_account.data.borrow())?;
        if lending_pool_state.status != LENDINGPOOL_OPEN {
            return Err(LendingPlatformError::InvalidAccounts.into());
        }

        let mut loan_state = LoanState::unpack(&loan_account.data.borrow())?;

        if loan_state.lending_account != *lending_pool_account.key {
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

        let current_time = clock.unix_timestamp;
        if current_time < 0 {
            return Err(LendingPlatformError::InvalidAccounts.into());
        }
        if (loan_state.borrowed_on + loan_state.validity_till) < current_time as u64 {
            return Err(LendingPlatformError::InvalidAccounts.into());
        }

        let transfer_amount = loan_state.collateral_lamports.try_into().map_err(|_e| LendingPlatformError::InvalidAccounts)?;
        let collateral_transfer_instruction = system_transfer(loan_account.key, lending_pool_account.key,  transfer_amount);
        invoke_signed(
            &collateral_transfer_instruction,
            &[
                loan_account.clone(),
                lending_pool_account.clone()
            ],
            &[&loan_account_pda_signer_seeds]
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
        let lending_pool_account = next_account_info(account_info_iter)?;
        let lending_pool_spl_account = next_account_info(account_info_iter)?;
        let deposit_spl_account = next_account_info(account_info_iter)?;
        let spl_mint_account = next_account_info(account_info_iter)?;
        let default_account = next_account_info(account_info_iter)?;
        let token_program = next_account_info(account_info_iter)?;

        if !lender_account.is_signer {
            return Err(LendingPlatformError::IncorrectSigner.into());
        }

        let lender_account_bytes = lender_account.key.to_bytes();
        let lending_pool_pda_signer_seeds : &[&[_]] = &[
            b"lending_pool",
            &lender_account_bytes,
            &[arg.lending_pool_bump_seed]
        ];
        let pda_lending_pool = Pubkey::create_program_address(lending_pool_pda_signer_seeds, program_id)?;

        if *lending_pool_account.key != pda_lending_pool {
            return Err(LendingPlatformError::InvalidAccounts.into());
        }

        let mut lending_pool_state = LendingPoolState::unpack(&lending_pool_account.data.borrow())?;
        if lending_pool_state.lender != *lender_account.key {
            return Err(LendingPlatformError::InvalidAccounts.into());
        }
        if lending_pool_state.number_of_outstanding_loans > 0 {
            return Err(LendingPlatformError::InvalidAccounts.into());
        }

        let lending_pool_spl_data = spl_token::state::Account::unpack(&lending_pool_spl_account.data.borrow())?;
        if lending_pool_spl_data.owner != *lending_pool_account.key {
            return Err(LendingPlatformError::InvalidAccounts.into());
        }
        if lending_pool_spl_data.mint != *spl_mint_account.key {
            return Err(LendingPlatformError::InvalidAccounts.into());
        }

        let deposit_account_spl_data = spl_token::state::Account::unpack(&deposit_spl_account.data.borrow())?;
        if deposit_account_spl_data.mint != *spl_mint_account.key {
            return Err(LendingPlatformError::InvalidAccounts.into());
        }

        let transfer_amount = lending_pool_state.collateral_amount.try_into().map_err(|_e| LendingPlatformError::InvalidAccounts)?;
        let collateral_transfer_instruction = system_transfer(lending_pool_account.key, default_account.key, transfer_amount);
        invoke_signed(
            &collateral_transfer_instruction,
            &[
                lending_pool_account.clone(),
                default_account.clone()
            ],
            &[&lending_pool_pda_signer_seeds]
        )?;

        let stablecoin_transfer_instruction = spl_token::instruction::transfer(
            token_program.key,
            lending_pool_spl_account.key,
            deposit_spl_account.key,
            lending_pool_account.key,
            &[],
            lending_pool_state.coin_amount as u64
        )?;
        invoke_signed(
            &stablecoin_transfer_instruction,
            &[
                token_program.clone(),
                lending_pool_spl_account.clone(),
                deposit_spl_account.clone(),
                lending_pool_account.clone()
            ],
            &[&lending_pool_pda_signer_seeds]
        )?;

        lending_pool_state.status = LENDINGPOOL_CLOSE;
        LendingPoolState::pack(lending_pool_state, &mut lending_pool_account.data.borrow_mut())?;
        Ok(())
    }
}
