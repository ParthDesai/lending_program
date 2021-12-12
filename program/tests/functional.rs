#![cfg(feature = "test-bpf")]

use std::str::FromStr;

use solana_program::{hash::Hash, pubkey::Pubkey, rent::Rent, sysvar};
use solana_program::instruction::InstructionError;
use solana_program_test::{processor, ProgramTest};
use solana_sdk::{account::Account, signature::Keypair, signature::Signer, system_instruction, transaction::Transaction};
use solana_sdk::program_pack::Pack;
use solana_sdk::transaction::TransactionError;
use spl_token::{self, instruction::{initialize_mint, initialize_account, mint_to}, state};
use lending_program::params::NewLendingPool;
use lending_program::entrypoint::process_instruction;
use lending_program::instructions::new_lending_pool;
use chainlink::state::{Aggregator, Config};
use solana_program::borsh::get_packed_len;
use borsh::ser::BorshSerialize;
use solana_sdk::system_transaction::create_account;
use lending_program::state::LendingPoolState;


#[tokio::test]
async fn test_lending_program() {

    // Create program and test environment
    let sol_to_usd_feed_pubkey = Pubkey::from_str("FmAmfoyPXiA8Vhhe6MZTr3U6rZfEZ1ctEHay1ysqCqcf").unwrap();
    let token_to_usd_feed_pubkey = Pubkey::from_str("DinfGKkKxJsU3kFnj173zSRdXNhZxxgZY8YC5GCQYhsi").unwrap();
    let program_id = Pubkey::from_str("VestingbGKPFXCWuBvfkegQfZyiNwAJb9Ss623VQ5DA").unwrap();

    let mint_authority = Keypair::new();
    let mint = Keypair::new();

    let lender_account = Keypair::new();
    let lender_token_account = Keypair::new();

    let borrower_account = Keypair::new();
    let borrower_token_account = Keypair::new();

    let mut arg = NewLendingPool{
        total_lending_amount: 500,
        max_payback_time: 400,
        expected_apy: 10,
        bump_seed: 0
    };
    let lending_pool_pda_signer_seeds : &[&[_]] = &[
        b"lending_pool",
        &lender_account.pubkey().to_bytes(),
    ];
    let (pda_lending_pool_address, bump_seed) = Pubkey::find_program_address(lending_pool_pda_signer_seeds, &program_id);
    arg.bump_seed = bump_seed;

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
        answer: Some(120u128),
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
        lender_account.pubkey(),
        Account {
            lamports: 50000000,
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
        answer: Some(240u128),
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
    let program_test_context = program_test.start_with_context().await;
    let mut banks_client = program_test_context.banks_client;
    let payer = program_test_context.payer;
    let recent_blockhash = program_test_context.last_blockhash;

    banks_client.process_transaction(mint_init_transaction(
        &payer,
        &mint,
        &mint_authority,
        recent_blockhash
    )).await.unwrap();

    banks_client.process_transaction(
        create_token_account(&payer, &mint, recent_blockhash, &lender_token_account, &lender_account.pubkey())
    ).await.unwrap();

    banks_client.process_transaction(
        create_token_account(&payer, &mint, recent_blockhash, &borrower_token_account, &borrower_account.pubkey())
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
        Some(&payer.pubkey()),
    );
    setup_transaction.partial_sign(
        &[
            &payer,
            &mint_authority
        ],
        recent_blockhash
    );

    banks_client.process_transaction(setup_transaction).await.unwrap();


    // Testing creating new lending pool
    let create_lending_pool_instruction = new_lending_pool(
        &program_id,
        arg,
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
        Some(&payer.pubkey())
    );

    create_lending_pool_transaction.partial_sign(
        &[
            &payer,
            &lender_account
        ],
        recent_blockhash
    );

    banks_client.process_transaction(create_lending_pool_transaction).await.unwrap();

    let lending_pool_account = banks_client.get_account(pda_lending_pool_address).await.unwrap().unwrap();
    let lending_pool_state = LendingPoolState::unpack(&lending_pool_account.data[..LendingPoolState::LEN]).unwrap();
    if !lending_pool_state.initialized {
        panic!("Lending pool state need to be initialized");
    }
    if lending_pool_state.lender != lender_account.pubkey() {
        panic!("Lending pool state's lender need to be correctly set. Expected: {}, Found: {}", lender_account.pubkey().to_string(), lending_pool_state.lender.to_string());
    }
    if lending_pool_state.expected_apy != arg.expected_apy {
        panic!("Expected apy is not matching");
    }
    if lending_pool_state.max_payback_time != arg.max_payback_time {
        panic!("Max payback time is not correct");
    }
    if lending_pool_state.coin_amount != arg.total_lending_amount as u128 {
        panic!("Lending pool state's coin amount is: {}, expected: {}", lending_pool_state.coin_amount, arg.total_lending_amount);
    }

    let lending_pool_spl_account = banks_client.get_account(lending_pool_token_account.pubkey()).await.unwrap().unwrap();
    let lending_pool_spl_account_state = spl_token::state::Account::unpack(&lending_pool_spl_account.data[..spl_token::state::Account::LEN]).unwrap();
    if lending_pool_spl_account_state.amount != arg.total_lending_amount {
        panic!("Lending pool spl account must have lending amount allocated");
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