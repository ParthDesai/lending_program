#![cfg(feature = "test-bpf")]

use std::str::FromStr;
use bincode;

use solana_program::{hash::Hash, pubkey::Pubkey, rent::Rent, sysvar};
use solana_program::instruction::InstructionError;
use solana_program_test::{processor, ProgramTest, ProgramTestBanksClientExt};
use solana_sdk::{account::Account, signature::Keypair, signature::Signer, system_instruction, transaction::Transaction};
use solana_sdk::program_pack::Pack;
use solana_sdk::transaction::TransactionError;
use spl_token::{self, instruction::{initialize_mint, initialize_account, mint_to}, state};
use lending_program::params::{CloseLending, DefaultLoan, InitLendingPoolAccount, InitLoanAccount, NewLendingPool, NewLoan, PaybackLoan};
use lending_program::entrypoint::process_instruction;
use lending_program::instructions::{close_lending, default_loan, init_lending_pool_account_data, init_loan_account_data, new_lending_pool, new_loan, payback_loan};
use chainlink::state::{Aggregator, Config};
use solana_program::borsh::get_packed_len;
use borsh::ser::BorshSerialize;
use solana_program::clock::Clock;
use solana_program::program_error::ProgramError;
use solana_sdk::system_transaction::create_account;
use solana_sdk::timing::SECONDS_PER_YEAR;
use lending_program::state::{LENDINGPOOL_CLOSE, LENDINGPOOL_OPEN, LendingPoolState, LoanState, LOANSTATE_DEFAULTED, LOANSTATE_LOANED, LOANSTATE_PAYEDBACK};

/// This test will not work as the ProgramTest does not support resizing of the account
/// But it should work fine with real blockchain.
#[tokio::test]
async fn test_init_loan() {
    // Create program and test environment
    let program_id = Pubkey::from_str("VestingbGKPFXCWuBvfkegQfZyiNwAJb9Ss623VQ5DA").unwrap();

    let lender_account = Keypair::new();
    let lender_token_account = Keypair::new();

    let borrower_account = Keypair::new();

    let mint_authority = Keypair::new();
    let mint = Keypair::new();

    let lending_pool_pda_signer_seeds : &[&[_]] = &[
        b"lending_pool",
        &lender_token_account.pubkey().to_bytes(),
    ];
    let (pda_lending_pool_address, loan_bump_seed) = Pubkey::find_program_address(lending_pool_pda_signer_seeds, &program_id);

    let loan_pda_signer_seeds : &[&[_]] = &[
        b"loan_account",  &pda_lending_pool_address.to_bytes(),
        &borrower_account.pubkey().to_bytes(),
    ];
    let (pda_loan_address, loan_bump_seed) = Pubkey::find_program_address(loan_pda_signer_seeds, &program_id);

    let mut program_test = ProgramTest::new(
        "lending_program",
        program_id,
        processor!(process_instruction),
    );

    program_test.add_account(
        borrower_account.pubkey(),
        Account {
            lamports: 50000000,
            ..Account::default()
        }
    );

    program_test.add_account(
        pda_lending_pool_address,
        Account {
            lamports: 40000000,
            data: Vec::from([0u8; LendingPoolState::LEN]),
            owner: program_id,
            ..Account::default()
        }
    );

    let mut program_test_context = program_test.start_with_context().await;
    let recent_blockhash = program_test_context.last_blockhash.clone();

    program_test_context.banks_client.process_transaction(mint_init_transaction(
        &program_test_context.payer,
        &mint,
        &mint_authority,
        recent_blockhash
    )).await.unwrap();

    program_test_context.banks_client.process_transaction(
        create_token_account(&program_test_context.payer, &mint, recent_blockhash, &lender_token_account, &lender_account.pubkey())
    ).await.unwrap();


    let init_loan_account_instruction = init_loan_account_data(
        &program_id,
        InitLoanAccount{
            loan_bump_seed,
        },
        &solana_program::system_program::id(),
        &pda_lending_pool_address,
        &borrower_account.pubkey(),
        &pda_loan_address
    ).unwrap();

    let mut init_loan_account_tx = Transaction::new_with_payer(
        &[init_loan_account_instruction],
        Some(&program_test_context.payer.pubkey()),
    );
    init_loan_account_tx.partial_sign(
        &[
            &program_test_context.payer,
            &borrower_account
        ],
        recent_blockhash
    );

    program_test_context.banks_client.process_transaction(init_loan_account_tx).await.unwrap();
}


/// This test will not work as the ProgramTest does not support resizing of the account
/// But it should work fine with real blockchain.
#[tokio::test]
async fn test_init_lending_account() {
    // Create program and test environment
    let program_id = Pubkey::from_str("VestingbGKPFXCWuBvfkegQfZyiNwAJb9Ss623VQ5DA").unwrap();

    let lender_account = Keypair::new();
    let lender_token_account = Keypair::new();

    let mint_authority = Keypair::new();
    let mint = Keypair::new();

    let lending_pool_pda_signer_seeds : &[&[_]] = &[
        b"lending_pool",
        &lender_token_account.pubkey().to_bytes(),
    ];
    let (pda_lending_pool_address, bump_seed) = Pubkey::find_program_address(lending_pool_pda_signer_seeds, &program_id);

    let mut program_test = ProgramTest::new(
        "lending_program",
        program_id,
        processor!(process_instruction),
    );

    program_test.add_account(
        lender_account.pubkey(),
        Account {
            lamports: 50000000,
            ..Account::default()
        }
    );

    let mut program_test_context = program_test.start_with_context().await;
    let recent_blockhash = program_test_context.last_blockhash.clone();

    program_test_context.banks_client.process_transaction(mint_init_transaction(
        &program_test_context.payer,
        &mint,
        &mint_authority,
        recent_blockhash
    )).await.unwrap();

    program_test_context.banks_client.process_transaction(
        create_token_account(&program_test_context.payer, &mint, recent_blockhash, &lender_token_account, &lender_account.pubkey())
    ).await.unwrap();


    let init_lending_pool_instruction = init_lending_pool_account_data(
        &program_id,
        InitLendingPoolAccount{
            bump_seed
        },
        &solana_program::system_program::id(),
        &lender_account.pubkey(),
        &lender_token_account.pubkey(),
        &pda_lending_pool_address
    ).unwrap();

    let mut init_lending_pool_tx = Transaction::new_with_payer(
        &[init_lending_pool_instruction],
        Some(&program_test_context.payer.pubkey()),
    );
    init_lending_pool_tx.partial_sign(
        &[
            &program_test_context.payer,
            &lender_account
        ],
        recent_blockhash
    );

    program_test_context.banks_client.process_transaction(init_lending_pool_tx).await.unwrap();
}


#[tokio::test]
async fn test_lending_program_default_loan() {

    // Create program and test environment
    let sol_to_usd_feed_pubkey = Pubkey::from_str("FmAmfoyPXiA8Vhhe6MZTr3U6rZfEZ1ctEHay1ysqCqcf").unwrap();
    let token_to_usd_feed_pubkey = Pubkey::from_str("DinfGKkKxJsU3kFnj173zSRdXNhZxxgZY8YC5GCQYhsi").unwrap();
    let program_id = Pubkey::from_str("VestingbGKPFXCWuBvfkegQfZyiNwAJb9Ss623VQ5DA").unwrap();

    let total_lending_amount = 500;
    let loan_amount = 100;
    let sol_to_usd_rate = 120u64;
    let coin_to_usd_rate = 240u64;

    let collateral_lamports = (((((100 * 240) * 100) / 60) / sol_to_usd_rate) + 1) * 100000000;
    let borrower_initial_balance = 50000000000;

    let mint_authority = Keypair::new();
    let mint = Keypair::new();

    let lender_account = Keypair::new();
    let lender_token_account = Keypair::new();
    let collateral_deposit_account = Keypair::new();
    let spl_deposit_account = Keypair::new();

    let borrower_account = Keypair::new();
    let borrower_token_account = Keypair::new();

    let lending_pool_pda_signer_seeds : &[&[_]] = &[
        b"lending_pool",
        &lender_token_account.pubkey().to_bytes(),
    ];
    let (pda_lending_pool_address, bump_seed) = Pubkey::find_program_address(lending_pool_pda_signer_seeds, &program_id);


    let lending_pool_collateral_pda_signer_seeds : &[&[_]] = &[
        b"lending_pool_collateral",
        &lender_token_account.pubkey().to_bytes(),
    ];
    let (pda_lending_pool_collateral, lending_pool_collateral_bump_seed) = Pubkey::find_program_address(lending_pool_collateral_pda_signer_seeds, &program_id);

    let mut lending_pool_arg = NewLendingPool{
        total_lending_amount,
        max_payback_time: 4000,
        expected_apy: 10,
        bump_seed
    };

    let loan_pda_signer_seeds : &[&[_]] = &[
        b"loan_account",  &pda_lending_pool_address.to_bytes(),
        &borrower_account.pubkey().to_bytes(),
    ];
    let (pda_loan_address, loan_bump_seed) = Pubkey::find_program_address(loan_pda_signer_seeds, &program_id);

    let loan_account_collateral_pda_signer_seeds: &[&[_]]  = &[
        b"loan_account_collateral",  &pda_lending_pool_address.to_bytes(),
        &borrower_account.pubkey().to_bytes(),
    ];
    let (pda_loan_account_collateral, loan_account_collateral_bump_seed) = Pubkey::find_program_address(loan_account_collateral_pda_signer_seeds, &program_id);

    let  loan_arg = NewLoan {
        amount: loan_amount,
        lending_pool_bump_seed: lending_pool_arg.bump_seed,
        loan_bump_seed,
        loan_collateral_bump_seed: loan_account_collateral_bump_seed
    };

    let lending_pool_token_account = Keypair::new();


    let mut program_test = ProgramTest::new(
        "lending_program",
        program_id,
        processor!(process_instruction),
    );

    let sol_to_usd_feed_data = Aggregator{
        is_initialized: true,
        version: 1,
        config: Config {
            oracles: vec![],
            min_answer_threshold: 0,
            staleness_threshold: 0,
            decimals: 0
        },
        updated_at: 0,
        owner: Default::default(),
        submissions: Default::default(),
        answer: Some(sol_to_usd_rate as u128),
    };
    let mut byte_vec = vec![0u8; 4096];
    let mut bytes = byte_vec.as_mut_slice();
    sol_to_usd_feed_data.serialize(&mut bytes).unwrap();
    program_test.add_account(
        sol_to_usd_feed_pubkey,
        Account {
            lamports: 50000000,
            data: byte_vec,
            owner: chainlink::id(),
            ..Account::default()
        }
    );


    program_test.add_account(
        pda_lending_pool_address,
        Account {
            lamports: 40000000,
            data: Vec::from([0u8; LendingPoolState::LEN]),
            owner: program_id,
            ..Account::default()
        }
    );

    program_test.add_account(
        pda_loan_address,
        Account {
            lamports: 40000000,
            data: Vec::from([0u8; LoanState::LEN]),
            owner: program_id,
            ..Account::default()
        }
    );

    program_test.add_account(
        lender_account.pubkey(),
        Account {
            lamports: 50000000,
            ..Account::default()
        }
    );

    program_test.add_account(
        borrower_account.pubkey(),
        Account {
            lamports: borrower_initial_balance,
            ..Account::default()
        }
    );

    program_test.add_account(
        lending_pool_token_account.pubkey(),
        Account {
            lamports: 50000000,
            data: Vec::from([0u8; spl_token::state::Account::LEN]),
            owner: spl_token::id(),
            ..Account::default()
        }
    );

    let token_to_usd_feed_data = Aggregator{
        is_initialized: true,
        version: 1,
        config: Config {
            oracles: vec![],
            min_answer_threshold: 0,
            staleness_threshold: 0,
            decimals: 0
        },
        updated_at: 0,
        owner: Default::default(),
        submissions: Default::default(),
        answer: Some(coin_to_usd_rate as u128),
    };
    let mut byte_vec = vec![0u8; 4096];
    let mut bytes = byte_vec.as_mut_slice();
    token_to_usd_feed_data.serialize(&mut bytes).unwrap();
    program_test.add_account(
        token_to_usd_feed_pubkey,
        Account {
            lamports: 50000000,
            data: byte_vec,
            owner: chainlink::id(),
            ..Account::default()
        }
    );


    // Start and process transactions on the test network
    let mut program_test_context = program_test.start_with_context().await;
    let recent_blockhash = program_test_context.last_blockhash.clone();

    program_test_context.warp_to_slot(100).unwrap();

    program_test_context.banks_client.process_transaction(mint_init_transaction(
        &program_test_context.payer,
        &mint,
        &mint_authority,
        recent_blockhash
    )).await.unwrap();

    program_test_context.banks_client.process_transaction(
        create_token_account(&program_test_context.payer, &mint, recent_blockhash, &lender_token_account, &lender_account.pubkey())
    ).await.unwrap();

    program_test_context.banks_client.process_transaction(
        create_token_account(&program_test_context.payer, &mint, recent_blockhash, &borrower_token_account, &borrower_account.pubkey())
    ).await.unwrap();

    program_test_context.banks_client.process_transaction(
        create_token_account(&program_test_context.payer, &mint, recent_blockhash, &spl_deposit_account, &collateral_deposit_account.pubkey())
    ).await.unwrap();

    let setup_instructions = [
        mint_to(
            &spl_token::id(),
            &mint.pubkey(),
            &lender_token_account.pubkey(),
            &mint_authority.pubkey(),
            &[],
            50000
        ).unwrap(),
    ];

    let mut setup_transaction = Transaction::new_with_payer(
        &setup_instructions,
        Some(&program_test_context.payer.pubkey()),
    );
    setup_transaction.partial_sign(
        &[
            &program_test_context.payer,
            &mint_authority
        ],
        recent_blockhash
    );

    program_test_context.banks_client.process_transaction(setup_transaction).await.unwrap();


    // Testing creating new lending pool
    let create_lending_pool_instruction = new_lending_pool(
        &program_id,
        lending_pool_arg,
        &lender_account.pubkey(),
        &lender_token_account.pubkey(),
        &pda_lending_pool_address,
        &lending_pool_token_account.pubkey(),
        &token_to_usd_feed_pubkey,
        &mint.pubkey(),
        &spl_token::id(),
    ).unwrap();

    let mut create_lending_pool_transaction = Transaction::new_with_payer(
        &[create_lending_pool_instruction],
        Some(&program_test_context.payer.pubkey())
    );

    create_lending_pool_transaction.partial_sign(
        &[
            &program_test_context.payer,
            &lender_account
        ],
        recent_blockhash
    );

    program_test_context.banks_client.process_transaction(create_lending_pool_transaction).await.unwrap();

    let lending_pool_account = program_test_context.banks_client.get_account(pda_lending_pool_address).await.unwrap().unwrap();
    let lending_pool_state = LendingPoolState::unpack(&lending_pool_account.data[..LendingPoolState::LEN]).unwrap();
    if !lending_pool_state.initialized {
        panic!("Lending pool state need to be initialized");
    }
    if lending_pool_state.lender != lender_account.pubkey() {
        panic!("Lending pool state's lender need to be correctly set. Expected: {}, Found: {}", lender_account.pubkey().to_string(), lending_pool_state.lender.to_string());
    }
    if lending_pool_state.expected_apy != lending_pool_arg.expected_apy {
        panic!("Expected apy is not matching");
    }
    if lending_pool_state.max_payback_time != lending_pool_arg.max_payback_time {
        panic!("Max payback time is not correct");
    }
    if lending_pool_state.coin_amount != lending_pool_arg.total_lending_amount as u128 {
        panic!("Lending pool state's coin amount is: {}, expected: {}", lending_pool_state.coin_amount, lending_pool_arg.total_lending_amount);
    }
    if lending_pool_state.number_of_outstanding_loans != 0 {
        panic!("Loan count should have been 0");
    }
    if lending_pool_state.status != LENDINGPOOL_OPEN {
        panic!("Lending pool should be open");
    }

    let lending_pool_spl_account = program_test_context.banks_client.get_account(lending_pool_token_account.pubkey()).await.unwrap().unwrap();
    let lending_pool_spl_account_state = spl_token::state::Account::unpack(&lending_pool_spl_account.data[..spl_token::state::Account::LEN]).unwrap();
    if lending_pool_spl_account_state.amount != lending_pool_arg.total_lending_amount {
        panic!("Lending pool spl account must have lending amount allocated");
    }

    let new_loan_instructions = new_loan(
        &program_id,
        loan_arg,
        &lender_token_account.pubkey(),
        &borrower_account.pubkey(),
        &borrower_token_account.pubkey(),
        &pda_lending_pool_address,
        &lending_pool_token_account.pubkey(),
        &pda_loan_address,
        &pda_loan_account_collateral,
        &mint.pubkey(),
        &token_to_usd_feed_pubkey,
        &sol_to_usd_feed_pubkey,
        &spl_token::id(),
        &solana_program::system_program::id()
    ).unwrap();

    let mut create_loan_transaction = Transaction::new_with_payer(
        &[new_loan_instructions],
        Some(&program_test_context.payer.pubkey())
    );

    create_loan_transaction.partial_sign(
        &[
            &program_test_context.payer,
            &borrower_account
        ],
        recent_blockhash
    );

    program_test_context.banks_client.process_transaction(create_loan_transaction).await.unwrap();

    let lending_pool_spl_account = program_test_context.banks_client.get_account(lending_pool_token_account.pubkey()).await.unwrap().unwrap();
    let lending_pool_spl_account_state = spl_token::state::Account::unpack(&lending_pool_spl_account.data[..spl_token::state::Account::LEN]).unwrap();
    if lending_pool_spl_account_state.amount != (lending_pool_arg.total_lending_amount - loan_amount) {
        panic!("Lending pool spl account must have lending amount allocated");
    }


    let loan_account = program_test_context.banks_client.get_account(pda_loan_address).await.unwrap().unwrap();
    let loan_state = LoanState::unpack(&loan_account.data[..LoanState::LEN]).unwrap();
    if !loan_state.initialized {
        panic!("Loan state is not initialized");
    }
    if loan_state.borrower != borrower_account.pubkey() {
        panic!("Loan state has incorrect lender set");
    }
    if loan_state.amount != loan_arg.amount {
        panic!("Coin amount in loan state does not match actual loan arg");
    }
    if loan_state.collateral_lamports != collateral_lamports as u128 {
        panic!("Collateral amount in loan state does not match");
    }
    if loan_state.status != LOANSTATE_LOANED {
        panic!("Loan should have been open");
    }


    let borrower_token_account = program_test_context.banks_client.get_account(borrower_token_account.pubkey()).await.unwrap().unwrap();
    let borrower_token_account_state = spl_token::state::Account::unpack(&borrower_token_account.data[..spl_token::state::Account::LEN]).unwrap();
    if borrower_token_account_state.amount != loan_arg.amount {
        panic!("Lending pool spl account must have lending amount allocated");
    }


    let borrower_account_balance = program_test_context.banks_client.get_balance(borrower_account.pubkey()).await.unwrap();
    if borrower_account_balance != borrower_initial_balance - collateral_lamports {
        panic!("Borrower account balance is not correct");
    }

    let lending_pool_account = program_test_context.banks_client.get_account(pda_lending_pool_address).await.unwrap().unwrap();
    let new_lending_pool_state = LendingPoolState::unpack(&lending_pool_account.data[..LendingPoolState::LEN]).unwrap();
    if new_lending_pool_state.number_of_outstanding_loans != 1 {
        panic!("Number of outstanding loan should be 1");
    }
    if new_lending_pool_state.coin_amount != lending_pool_state.coin_amount - loan_state.amount as u128 {
        panic!("Coin amount should be initial amount minus loan state amount");
    }

    let loan_collateral_balance = program_test_context.banks_client.get_balance(pda_loan_account_collateral).await.unwrap();
    if loan_collateral_balance != collateral_lamports {
        panic!("Loan collateral's balance does not match collateral lamport account");
    }

    program_test_context.warp_to_slot(100000).unwrap();

    let default_loan_instruction = default_loan(
        &program_id,
        DefaultLoan { lending_pool_bump_seed: lending_pool_arg.bump_seed, loan_bump_seed, lending_pool_collateral_seed: lending_pool_collateral_bump_seed, loan_collateral_seed: loan_account_collateral_bump_seed },
        &lender_token_account.pubkey(),
        &pda_lending_pool_address,
        &pda_lending_pool_collateral,
        &pda_loan_address,
        &pda_loan_account_collateral,
        &borrower_account.pubkey(),
        &solana_program::system_program::id()
    ).unwrap();


    let mut default_loan_transaction = Transaction::new_with_payer(
        &[default_loan_instruction],
        Some(&program_test_context.payer.pubkey())
    );

    default_loan_transaction.partial_sign(
        &[
            &program_test_context.payer,
        ],
        recent_blockhash
    );

    program_test_context.banks_client.process_transaction(default_loan_transaction).await.unwrap();

    let lending_pool_account = program_test_context.banks_client.get_account(pda_lending_pool_address).await.unwrap().unwrap();
    let defaulted_lending_pool_state = LendingPoolState::unpack(&lending_pool_account.data[..LendingPoolState::LEN]).unwrap();
    if defaulted_lending_pool_state.number_of_outstanding_loans != 0 {
        panic!("There should not be any outstanding loans");
    }
    if defaulted_lending_pool_state.coin_amount != lending_pool_state.coin_amount - loan_state.amount as u128 {
        panic!("Defaulted lending pool should not get any tokens back.");
    }
    if defaulted_lending_pool_state.collateral_amount != collateral_lamports as u128 {
        panic!("Defaulted lending pool should have collateral amount");
    }
    if defaulted_lending_pool_state.status != LENDINGPOOL_OPEN {
        panic!("Lending pool should have been open");
    }

    let loan_collateral_balance = program_test_context.banks_client.get_balance(pda_loan_account_collateral).await.unwrap();
    if loan_collateral_balance != 0 {
        panic!("Loan collateral's balance should be zero as it should be transferred to the lending pool collateral");
    }

    let lending_pool_collateral_balance = program_test_context.banks_client.get_balance(pda_lending_pool_collateral).await.unwrap();
    if lending_pool_collateral_balance != collateral_lamports {
        panic!("Lending pool collateral balance should be equal to collateral");
    }

    let loan_account = program_test_context.banks_client.get_account(pda_loan_address).await.unwrap().unwrap();
    let loan_state = LoanState::unpack(&loan_account.data[..LoanState::LEN]).unwrap();
    if !loan_state.initialized {
        panic!("Loan state is not initialized");
    }
    if loan_state.borrower != borrower_account.pubkey() {
        panic!("Loan state has incorrect lender set");
    }
    if loan_state.amount != loan_arg.amount {
        panic!("Coin amount in loan state does not match actual loan arg");
    }
    if loan_state.collateral_lamports != collateral_lamports as u128 {
        panic!("Collateral amount in loan state does not match");
    }
    if loan_state.status != LOANSTATE_DEFAULTED {
        panic!("Loan state should be defaulted");
    }

    let close_lending_instruction = close_lending(
        &program_id,
        CloseLending {
            lending_pool_bump_seed: lending_pool_arg.bump_seed,
            lending_pool_collateral_seed: lending_pool_collateral_bump_seed
        },
        &lender_account.pubkey(),
        &lender_token_account.pubkey(),
        &pda_lending_pool_address,
        &lending_pool_token_account.pubkey(),
        &pda_lending_pool_collateral,
        &spl_deposit_account.pubkey(),
        &mint.pubkey(),
        &collateral_deposit_account.pubkey(),
        &spl_token::id(),
        &solana_program::system_program::id()
    ).unwrap();

    let mut close_lending_account_transaction = Transaction::new_with_payer(
        &[close_lending_instruction],
        Some(&program_test_context.payer.pubkey())
    );

    close_lending_account_transaction.partial_sign(
        &[
            &program_test_context.payer,
            &lender_account
        ],
        recent_blockhash
    );

    program_test_context.banks_client.process_transaction(close_lending_account_transaction).await.unwrap();

    let lending_pool_collateral_balance = program_test_context.banks_client.get_balance(pda_lending_pool_collateral).await.unwrap();
    if lending_pool_collateral_balance != 0 {
        panic!("Lending pool collateral balance should be equal to zero");
    }

    let collateral_depost_account_balance = program_test_context.banks_client.get_balance(collateral_deposit_account.pubkey()).await.unwrap();
    if collateral_depost_account_balance != collateral_lamports {
        panic!("Collateral deposit balance should be equal to collateral lamports");
    }

    let spl_deposit_account = program_test_context.banks_client.get_account(spl_deposit_account.pubkey()).await.unwrap().unwrap();
    let spl_deposit_account_state = spl_token::state::Account::unpack(&spl_deposit_account.data[..spl_token::state::Account::LEN]).unwrap();
    if spl_deposit_account_state.amount != lending_pool_arg.total_lending_amount - loan_arg.amount {
        panic!("Spl deposit amount must be equal to lending pool's total lending amount - loaned amount");
    }

    let lending_pool_account = program_test_context.banks_client.get_account(pda_lending_pool_address).await.unwrap().unwrap();
    let closed_lending_pool_state = LendingPoolState::unpack(&lending_pool_account.data[..LendingPoolState::LEN]).unwrap();
    if closed_lending_pool_state.number_of_outstanding_loans != 0 {
        panic!("There should not be any outstanding loans");
    }
    if closed_lending_pool_state.coin_amount != lending_pool_state.coin_amount - loan_state.amount as u128 {
        panic!("Defaulted lending pool should not get any tokens back.");
    }
    if closed_lending_pool_state.collateral_amount != collateral_lamports as u128 {
        panic!("Defaulted lending pool should have collateral amount");
    }
    if closed_lending_pool_state.status != LENDINGPOOL_CLOSE {
        panic!("Lending pool should have been closed");
    }

}

#[tokio::test]
async fn test_lending_program_payback_loan() {

    // Create program and test environment
    let sol_to_usd_feed_pubkey = Pubkey::from_str("FmAmfoyPXiA8Vhhe6MZTr3U6rZfEZ1ctEHay1ysqCqcf").unwrap();
    let token_to_usd_feed_pubkey = Pubkey::from_str("DinfGKkKxJsU3kFnj173zSRdXNhZxxgZY8YC5GCQYhsi").unwrap();
    let program_id = Pubkey::from_str("VestingbGKPFXCWuBvfkegQfZyiNwAJb9Ss623VQ5DA").unwrap();

    let total_lending_amount = 500;
    let loan_amount = 100;
    let sol_to_usd_rate = 120u64;
    let coin_to_usd_rate = 240u64;

    let collateral_lamports = (((((100 * 240) * 100) / 60) / sol_to_usd_rate) + 1) * 100000000;
    let borrower_initial_balance = 50000000000;

    let mint_authority = Keypair::new();
    let mint = Keypair::new();

    let lender_account = Keypair::new();
    let lender_token_account = Keypair::new();
    let collateral_deposit_account = Keypair::new();
    let spl_deposit_account = Keypair::new();

    let borrower_account = Keypair::new();
    let borrower_token_account = Keypair::new();

    let lending_pool_pda_signer_seeds : &[&[_]] = &[
        b"lending_pool",
        &lender_token_account.pubkey().to_bytes(),
    ];
    let (pda_lending_pool_address, bump_seed) = Pubkey::find_program_address(lending_pool_pda_signer_seeds, &program_id);


    let lending_pool_collateral_pda_signer_seeds : &[&[_]] = &[
        b"lending_pool_collateral",
        &lender_token_account.pubkey().to_bytes(),
    ];
    let (pda_lending_pool_collateral, lending_pool_collateral_bump_seed) = Pubkey::find_program_address(lending_pool_collateral_pda_signer_seeds, &program_id);

    let mut lending_pool_arg = NewLendingPool{
        total_lending_amount,
        max_payback_time: 40000,
        expected_apy: 50,
        bump_seed
    };

    let loan_pda_signer_seeds : &[&[_]] = &[
        b"loan_account",  &pda_lending_pool_address.to_bytes(),
        &borrower_account.pubkey().to_bytes(),
    ];
    let (pda_loan_address, loan_bump_seed) = Pubkey::find_program_address(loan_pda_signer_seeds, &program_id);

    let loan_account_collateral_pda_signer_seeds: &[&[_]]  = &[
        b"loan_account_collateral",  &pda_lending_pool_address.to_bytes(),
        &borrower_account.pubkey().to_bytes(),
    ];
    let (pda_loan_account_collateral, loan_account_collateral_bump_seed) = Pubkey::find_program_address(loan_account_collateral_pda_signer_seeds, &program_id);

    let  loan_arg = NewLoan {
        amount: loan_amount,
        lending_pool_bump_seed: lending_pool_arg.bump_seed,
        loan_bump_seed,
        loan_collateral_bump_seed: loan_account_collateral_bump_seed
    };

    let lending_pool_token_account = Keypair::new();


    let mut program_test = ProgramTest::new(
        "lending_program",
        program_id,
        processor!(process_instruction),
    );

    let sol_to_usd_feed_data = Aggregator{
        is_initialized: true,
        version: 1,
        config: Config {
            oracles: vec![],
            min_answer_threshold: 0,
            staleness_threshold: 0,
            decimals: 0
        },
        updated_at: 0,
        owner: Default::default(),
        submissions: Default::default(),
        answer: Some(sol_to_usd_rate as u128),
    };
    let mut byte_vec = vec![0u8; 4096];
    let mut bytes = byte_vec.as_mut_slice();
    sol_to_usd_feed_data.serialize(&mut bytes).unwrap();
    program_test.add_account(
        sol_to_usd_feed_pubkey,
        Account {
            lamports: 50000000,
            data: byte_vec,
            owner: chainlink::id(),
            ..Account::default()
        }
    );


    program_test.add_account(
        pda_lending_pool_address,
        Account {
            lamports: 40000000,
            data: Vec::from([0u8; LendingPoolState::LEN]),
            owner: program_id,
            ..Account::default()
        }
    );

    program_test.add_account(
        pda_loan_address,
        Account {
            lamports: 40000000,
            data: Vec::from([0u8; LoanState::LEN]),
            owner: program_id,
            ..Account::default()
        }
    );

    program_test.add_account(
        lender_account.pubkey(),
        Account {
            lamports: 50000000,
            ..Account::default()
        }
    );

    program_test.add_account(
        borrower_account.pubkey(),
        Account {
            lamports: borrower_initial_balance,
            ..Account::default()
        }
    );

    program_test.add_account(
        lending_pool_token_account.pubkey(),
        Account {
            lamports: 50000000,
            data: Vec::from([0u8; spl_token::state::Account::LEN]),
            owner: spl_token::id(),
            ..Account::default()
        }
    );

    let token_to_usd_feed_data = Aggregator{
        is_initialized: true,
        version: 1,
        config: Config {
            oracles: vec![],
            min_answer_threshold: 0,
            staleness_threshold: 0,
            decimals: 0
        },
        updated_at: 0,
        owner: Default::default(),
        submissions: Default::default(),
        answer: Some(coin_to_usd_rate as u128),
    };
    let mut byte_vec = vec![0u8; 4096];
    let mut bytes = byte_vec.as_mut_slice();
    token_to_usd_feed_data.serialize(&mut bytes).unwrap();
    program_test.add_account(
        token_to_usd_feed_pubkey,
        Account {
            lamports: 50000000,
            data: byte_vec,
            owner: chainlink::id(),
            ..Account::default()
        }
    );


    // Start and process transactions on the test network
    let mut program_test_context = program_test.start_with_context().await;
    let recent_blockhash = program_test_context.last_blockhash.clone();

    program_test_context.warp_to_slot(100).unwrap();

    program_test_context.banks_client.process_transaction(mint_init_transaction(
        &program_test_context.payer,
        &mint,
        &mint_authority,
        recent_blockhash
    )).await.unwrap();

    program_test_context.banks_client.process_transaction(
        create_token_account(&program_test_context.payer, &mint, recent_blockhash, &lender_token_account, &lender_account.pubkey())
    ).await.unwrap();

    program_test_context.banks_client.process_transaction(
        create_token_account(&program_test_context.payer, &mint, recent_blockhash, &borrower_token_account, &borrower_account.pubkey())
    ).await.unwrap();

    program_test_context.banks_client.process_transaction(
        create_token_account(&program_test_context.payer, &mint, recent_blockhash, &spl_deposit_account, &collateral_deposit_account.pubkey())
    ).await.unwrap();

    let setup_instructions = [
        mint_to(
            &spl_token::id(),
            &mint.pubkey(),
            &lender_token_account.pubkey(),
            &mint_authority.pubkey(),
            &[],
            50000
        ).unwrap(),
    ];

    let mut setup_transaction = Transaction::new_with_payer(
        &setup_instructions,
        Some(&program_test_context.payer.pubkey()),
    );
    setup_transaction.partial_sign(
        &[
            &program_test_context.payer,
            &mint_authority
        ],
        recent_blockhash
    );

    program_test_context.banks_client.process_transaction(setup_transaction).await.unwrap();


    // Testing creating new lending pool
    let create_lending_pool_instruction = new_lending_pool(
        &program_id,
        lending_pool_arg,
        &lender_account.pubkey(),
        &lender_token_account.pubkey(),
        &pda_lending_pool_address,
        &lending_pool_token_account.pubkey(),
        &token_to_usd_feed_pubkey,
        &mint.pubkey(),
        &spl_token::id(),
    ).unwrap();

    let mut create_lending_pool_transaction = Transaction::new_with_payer(
        &[create_lending_pool_instruction],
        Some(&program_test_context.payer.pubkey())
    );

    create_lending_pool_transaction.partial_sign(
        &[
            &program_test_context.payer,
            &lender_account
        ],
        recent_blockhash
    );

    program_test_context.banks_client.process_transaction(create_lending_pool_transaction).await.unwrap();

    let lending_pool_account = program_test_context.banks_client.get_account(pda_lending_pool_address).await.unwrap().unwrap();
    let lending_pool_state = LendingPoolState::unpack(&lending_pool_account.data[..LendingPoolState::LEN]).unwrap();
    if !lending_pool_state.initialized {
        panic!("Lending pool state need to be initialized");
    }
    if lending_pool_state.lender != lender_account.pubkey() {
        panic!("Lending pool state's lender need to be correctly set. Expected: {}, Found: {}", lender_account.pubkey().to_string(), lending_pool_state.lender.to_string());
    }
    if lending_pool_state.expected_apy != lending_pool_arg.expected_apy {
        panic!("Expected apy is not matching");
    }
    if lending_pool_state.max_payback_time != lending_pool_arg.max_payback_time {
        panic!("Max payback time is not correct");
    }
    if lending_pool_state.coin_amount != lending_pool_arg.total_lending_amount as u128 {
        panic!("Lending pool state's coin amount is: {}, expected: {}", lending_pool_state.coin_amount, lending_pool_arg.total_lending_amount);
    }
    if lending_pool_state.number_of_outstanding_loans != 0 {
        panic!("Loan count should have been 0");
    }

    let lending_pool_spl_account = program_test_context.banks_client.get_account(lending_pool_token_account.pubkey()).await.unwrap().unwrap();
    let lending_pool_spl_account_state = spl_token::state::Account::unpack(&lending_pool_spl_account.data[..spl_token::state::Account::LEN]).unwrap();
    if lending_pool_spl_account_state.amount != lending_pool_arg.total_lending_amount {
        panic!("Lending pool spl account must have lending amount allocated");
    }

    let new_loan_instructions = new_loan(
        &program_id,
        loan_arg,
        &lender_token_account.pubkey(),
        &borrower_account.pubkey(),
        &borrower_token_account.pubkey(),
        &pda_lending_pool_address,
        &lending_pool_token_account.pubkey(),
        &pda_loan_address,
        &pda_loan_account_collateral,
        &mint.pubkey(),
        &token_to_usd_feed_pubkey,
        &sol_to_usd_feed_pubkey,
        &spl_token::id(),
        &solana_program::system_program::id()
    ).unwrap();

    let mut create_loan_transaction = Transaction::new_with_payer(
        &[new_loan_instructions],
        Some(&program_test_context.payer.pubkey())
    );

    create_loan_transaction.partial_sign(
        &[
            &program_test_context.payer,
            &borrower_account
        ],
        recent_blockhash
    );

    program_test_context.banks_client.process_transaction(create_loan_transaction).await.unwrap();

    let lending_pool_spl_account = program_test_context.banks_client.get_account(lending_pool_token_account.pubkey()).await.unwrap().unwrap();
    let lending_pool_spl_account_state = spl_token::state::Account::unpack(&lending_pool_spl_account.data[..spl_token::state::Account::LEN]).unwrap();
    if lending_pool_spl_account_state.amount != (lending_pool_arg.total_lending_amount - loan_amount) {
        panic!("Lending pool spl account must have lending amount allocated");
    }


    let loan_account = program_test_context.banks_client.get_account(pda_loan_address).await.unwrap().unwrap();
    let loan_state = LoanState::unpack(&loan_account.data[..LoanState::LEN]).unwrap();
    if !loan_state.initialized {
        panic!("Loan state is not initialized");
    }
    if loan_state.borrower != borrower_account.pubkey() {
        panic!("Loan state has incorrect lender set");
    }
    if loan_state.amount != loan_arg.amount {
        panic!("Coin amount in loan state does not match actual loan arg");
    }
    if loan_state.collateral_lamports != collateral_lamports as u128 {
        panic!("Collateral amount in loan state does not match");
    }
    if loan_state.status != LOANSTATE_LOANED {
        panic!("Loan should have been payed back");
    }


    let borrower_token_account_data = program_test_context.banks_client.get_account(borrower_token_account.pubkey()).await.unwrap().unwrap();
    let borrower_token_account_state = spl_token::state::Account::unpack(&borrower_token_account_data.data[..spl_token::state::Account::LEN]).unwrap();
    if borrower_token_account_state.amount != loan_arg.amount {
        panic!("Lending pool spl account must have lending amount allocated");
    }


    let borrower_account_balance = program_test_context.banks_client.get_balance(borrower_account.pubkey()).await.unwrap();
    if borrower_account_balance != borrower_initial_balance - collateral_lamports {
        panic!("Borrower account balance is not correct");
    }

    let lending_pool_account = program_test_context.banks_client.get_account(pda_lending_pool_address).await.unwrap().unwrap();
    let new_lending_pool_state = LendingPoolState::unpack(&lending_pool_account.data[..LendingPoolState::LEN]).unwrap();
    if new_lending_pool_state.number_of_outstanding_loans != 1 {
        panic!("Number of outstanding loan should be 1");
    }
    if new_lending_pool_state.coin_amount != lending_pool_state.coin_amount - loan_state.amount as u128 {
        panic!("Coin amount should be initial amount minus loan state amount");
    }

    let loan_collateral_balance = program_test_context.banks_client.get_balance(pda_loan_account_collateral).await.unwrap();
    if loan_collateral_balance != collateral_lamports {
        panic!("Loan collateral's balance does not match collateral lamport account");
    }

    program_test_context.warp_to_slot(100000).unwrap();

    // Before we payback we need to add 1 token extra
    let setup_instructions = [
        mint_to(
            &spl_token::id(),
            &mint.pubkey(),
            &borrower_token_account.pubkey(),
            &mint_authority.pubkey(),
            &[],
            1
        ).unwrap(),
    ];

    let mut setup_transaction = Transaction::new_with_payer(
        &setup_instructions,
        Some(&program_test_context.payer.pubkey()),
    );
    setup_transaction.partial_sign(
        &[
            &program_test_context.payer,
            &mint_authority
        ],
        recent_blockhash
    );

    program_test_context.banks_client.process_transaction(setup_transaction).await.unwrap();

    let payback_loan_instruction = payback_loan(
        &program_id,
        PaybackLoan {
            lending_pool_bump_seed: lending_pool_arg.bump_seed,
            loan_bump_seed,
            loan_collateral_bump_seed: loan_account_collateral_bump_seed
        },
        &lender_token_account.pubkey(),
        &borrower_account.pubkey(),
        &borrower_token_account.pubkey(),
        &pda_lending_pool_address,
        &lending_pool_token_account.pubkey(),
        &pda_loan_address,
        &pda_loan_account_collateral,
        &mint.pubkey(),
        &spl_token::id(),
        &solana_program::system_program::id()
    ).unwrap();

    let mut payback_loan_transaction = Transaction::new_with_payer(
        &[payback_loan_instruction],
        Some(&program_test_context.payer.pubkey())
    );

    payback_loan_transaction.partial_sign(
        &[
            &program_test_context.payer,
            &borrower_account
        ],
        recent_blockhash
    );

    program_test_context.banks_client.process_transaction(payback_loan_transaction).await.unwrap();

    let borrower_token_account_data = program_test_context.banks_client.get_account(borrower_token_account.pubkey()).await.unwrap().unwrap();
    let borrower_token_account_state = spl_token::state::Account::unpack(&borrower_token_account_data.data[..spl_token::state::Account::LEN]).unwrap();
    if borrower_token_account_state.amount != 0 {
        panic!("Borrower token account should have zero tokens");
    }

    let lending_pool_account = program_test_context.banks_client.get_account(pda_lending_pool_address).await.unwrap().unwrap();
    let payed_back_lending_pool_state = LendingPoolState::unpack(&lending_pool_account.data[..LendingPoolState::LEN]).unwrap();
    if payed_back_lending_pool_state.number_of_outstanding_loans != 0 {
        panic!("Number of outstanding loan should be 0");
    }
    if payed_back_lending_pool_state.coin_amount != lending_pool_state.coin_amount as u128 {
        panic!("Coin amount should be principle amount");
    }
    if payed_back_lending_pool_state.status != LENDINGPOOL_OPEN {
        panic!("Lending pool must be open");
    }

    let loan_account = program_test_context.banks_client.get_account(pda_loan_address).await.unwrap().unwrap();
    let loan_state = LoanState::unpack(&loan_account.data[..LoanState::LEN]).unwrap();
    if !loan_state.initialized {
        panic!("Loan state is not initialized");
    }
    if loan_state.borrower != borrower_account.pubkey() {
        panic!("Loan state has incorrect lender set");
    }
    if loan_state.amount != loan_arg.amount {
        panic!("Coin amount in loan state does not match actual loan arg");
    }
    if loan_state.collateral_lamports != collateral_lamports as u128 {
        panic!("Collateral amount in loan state does not match");
    }
    if loan_state.status != LOANSTATE_PAYEDBACK {
        panic!("Loan should have been payed back");
    }

    let lending_pool_spl_account = program_test_context.banks_client.get_account(lending_pool_token_account.pubkey()).await.unwrap().unwrap();
    let lending_pool_spl_account_state = spl_token::state::Account::unpack(&lending_pool_spl_account.data[..spl_token::state::Account::LEN]).unwrap();
    if lending_pool_spl_account_state.amount != lending_pool_arg.total_lending_amount {
        panic!("Lending pool spl account must have lending amount allocated");
    }

    let lender_spl_account = program_test_context.banks_client.get_account(lender_token_account.pubkey()).await.unwrap().unwrap();
    let lender_spl_account_state = spl_token::state::Account::unpack(&lender_spl_account.data[..spl_token::state::Account::LEN]).unwrap();
    if lender_spl_account_state.amount != (50000 - lending_pool_arg.total_lending_amount) + 1 {
        panic!("Lender should have received interest. Expected: {}, Actual: {}", (50000 - lending_pool_arg.total_lending_amount) + 1, lender_spl_account_state.amount);
    }

    let close_lending_instruction = close_lending(
        &program_id,
        CloseLending {
            lending_pool_bump_seed: lending_pool_arg.bump_seed,
            lending_pool_collateral_seed: lending_pool_collateral_bump_seed
        },
        &lender_account.pubkey(),
        &lender_token_account.pubkey(),
        &pda_lending_pool_address,
        &lending_pool_token_account.pubkey(),
        &pda_lending_pool_collateral,
        &spl_deposit_account.pubkey(),
        &mint.pubkey(),
        &collateral_deposit_account.pubkey(),
        &spl_token::id(),
        &solana_program::system_program::id()
    ).unwrap();

    let mut close_lending_account_transaction = Transaction::new_with_payer(
        &[close_lending_instruction],
        Some(&program_test_context.payer.pubkey())
    );

    close_lending_account_transaction.partial_sign(
        &[
            &program_test_context.payer,
            &lender_account
        ],
        recent_blockhash
    );

    program_test_context.banks_client.process_transaction(close_lending_account_transaction).await.unwrap();

    let lending_pool_collateral_balance = program_test_context.banks_client.get_balance(pda_lending_pool_collateral).await.unwrap();
    if lending_pool_collateral_balance != 0 {
        panic!("Lending pool collateral balance should be equal to zero");
    }

    let collateral_depost_account_balance = program_test_context.banks_client.get_balance(collateral_deposit_account.pubkey()).await.unwrap();
    if collateral_depost_account_balance != 0 {
        panic!("Collateral deposit balance should be zero since loan was payed back");
    }

    let spl_deposit_account = program_test_context.banks_client.get_account(spl_deposit_account.pubkey()).await.unwrap().unwrap();
    let spl_deposit_account_state = spl_token::state::Account::unpack(&spl_deposit_account.data[..spl_token::state::Account::LEN]).unwrap();
    if spl_deposit_account_state.amount != lending_pool_arg.total_lending_amount {
        panic!("Spl deposit amount must be equal to lending pool's total lending amount. Actual: {}", spl_deposit_account_state.amount);
    }

    let lending_pool_account = program_test_context.banks_client.get_account(pda_lending_pool_address).await.unwrap().unwrap();
    let closed_lending_pool = LendingPoolState::unpack(&lending_pool_account.data[..LendingPoolState::LEN]).unwrap();
    if closed_lending_pool.number_of_outstanding_loans != 0 {
        panic!("Number of outstanding loan should be 0");
    }
    if closed_lending_pool.status != LENDINGPOOL_CLOSE {
        panic!("Lending pool must be closed");
    }

}

fn mint_init_transaction(
    payer: &Keypair,
    mint:&Keypair,
    mint_authority: &Keypair,
    recent_blockhash: Hash) -> Transaction{
    let instructions = [
        system_instruction::create_account(
            &payer.pubkey(),
            &mint.pubkey(),
            Rent::default().minimum_balance(82),
            82,
            &spl_token::id()

        ),
        initialize_mint(
            &spl_token::id(),
            &mint.pubkey(),
            &mint_authority.pubkey(),
            None,
            0
        ).unwrap(),
    ];
    let mut transaction = Transaction::new_with_payer(
        &instructions,
        Some(&payer.pubkey()),
    );
    transaction.partial_sign(
        &[
            payer,
            mint
        ],
        recent_blockhash
    );
    transaction
}

fn create_token_account(
    payer: &Keypair,
    mint:&Keypair,
    recent_blockhash: Hash,
    token_account:&Keypair,
    token_account_owner: &Pubkey
) -> Transaction {
    let instructions = [
        system_instruction::create_account(
            &payer.pubkey(),
            &token_account.pubkey(),
            Rent::default().minimum_balance(165),
            165,
            &spl_token::id()
        ),
        initialize_account(
            &spl_token::id(),
            &token_account.pubkey(),
            &mint.pubkey(),
            token_account_owner
        ).unwrap()
    ];
    let mut transaction = Transaction::new_with_payer(
        &instructions,
        Some(&payer.pubkey()),
    );
    transaction.partial_sign(
        &[
            payer,
            token_account
        ],
        recent_blockhash
    );
    transaction
}