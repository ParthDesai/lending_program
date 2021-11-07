use football_nft::state::{PlayerNFTSet, PlayerNFTState};
use solana_client::rpc_client::RpcClient;
use solana_sdk::{
    pubkey::Pubkey,
    signature::{Keypair, Signer},
    system_instruction::create_account,
    transaction::Transaction,
};

use std::convert::TryInto;
use std::str::FromStr;

use football_nft::instructions::{
    change_nft_parameters, change_nft_set_admin, mint_new_nft, new_nft_set, transfer_nft,
};
use football_nft::params::{
    ChangeNFTParams, ChangeNFTSetAdminParams, MintNewNFTParams, NewNFTSetParams,
};
use spl_token::solana_program::program_option::COption;
use spl_token::solana_program::program_pack::Pack;
use spl_token::state::Mint;

use serde::{Deserialize, Serialize};

use std::collections::HashMap;
use std::{env, fs};

struct Config {
    nft_program_pubkey: Pubkey,
    token_program_pubkey: Pubkey,
    payer: Keypair,
    client: RpcClient,
    admin: Keypair,
}

#[derive(Serialize, Deserialize, Debug)]
struct ConfigDisk {
    nft_program_pubkey: String,
    token_program_pubkey: String,
    admin: String,
    payer: String,
    rpc_url: String,
}

#[derive(Debug)]
struct NFTSets {
    set_keys: Vec<Keypair>,
}

#[derive(Serialize, Deserialize, Debug)]
struct NFTSetsDisk {
    set_keys: Vec<String>,
}

#[derive(Debug)]
struct NFTInfo {
    nft_keypair: Keypair,
    spl_token_keypair: Keypair,
    spl_token_mint_keypair: Keypair,
    is_payer_owner: bool,
    set_id: Pubkey,
}

#[derive(Serialize, Deserialize, Debug)]
struct NFTInfoDisk {
    nft_keypair: String,
    spl_token_keypair: String,
    spl_token_mint_keypair: String,
    is_payer_owner: bool,
    set_id: String,
}

struct NFTsInfo {
    nfts: HashMap<Pubkey, HashMap<Pubkey, NFTInfo>>,
}

#[derive(Serialize, Deserialize, Debug)]
struct NFTsInfoDisk {
    nfts: Vec<NFTInfoDisk>,
}

const DEFAULT_DATA_PATH: &str = "./data";

fn read_config(data_dir: &String) -> Config {
    let data = fs::read_to_string(&format!("{}/config.json", data_dir)).unwrap();
    let config_disk: ConfigDisk = serde_json::from_str(data.as_str()).unwrap();
    Config {
        nft_program_pubkey: Pubkey::from_str(&config_disk.nft_program_pubkey).unwrap(),
        token_program_pubkey: Pubkey::from_str(&config_disk.token_program_pubkey).unwrap(),
        payer: Keypair::from_base58_string(&config_disk.payer),
        client: RpcClient::new(config_disk.rpc_url),
        admin: Keypair::from_base58_string(&config_disk.admin),
    }
}

fn read_sets(data_dir: &String) -> NFTSets {
    let data = fs::read_to_string(&format!("{}/sets.json", data_dir));
    if data.is_err() {
        println!("Did not find any sets. Initializing empty structure");
        return NFTSets { set_keys: vec![] };
    }
    let data = data.unwrap();
    let sets_disk: NFTSetsDisk = serde_json::from_str(data.as_str()).unwrap();
    let mut sets: Vec<Keypair> = vec![];
    for key in sets_disk.set_keys {
        sets.push(Keypair::from_base58_string(&key));
    }
    NFTSets { set_keys: sets }
}

fn write_sets(data_dir: &String, sets: &NFTSets) {
    let mut set_keys: Vec<String> = vec![];
    for set in &sets.set_keys {
        set_keys.push(set.to_base58_string());
    }
    let nft_set_disk = NFTSetsDisk { set_keys };
    let serialized_data = serde_json::to_string(&nft_set_disk).unwrap();
    fs::write(&format!("{}/sets.json", data_dir), serialized_data).unwrap();
}

fn read_nfts(data_dir: &String) -> NFTsInfo {
    let data = fs::read_to_string(&format!("{}/nfts.json", data_dir));
    if data.is_err() {
        println!("Did not find any nfts. Initializing empty structure");
        return NFTsInfo {
            nfts: HashMap::new(),
        };
    }
    let data = data.unwrap();
    let disk_nfts: NFTsInfoDisk = serde_json::from_str(&data).unwrap();
    let mut map: HashMap<Pubkey, HashMap<Pubkey, NFTInfo>> = HashMap::new();

    for disk_nft in disk_nfts.nfts {
        let set_id = Pubkey::from_str(&disk_nft.set_id).unwrap();
        let nft_keypair = Keypair::from_base58_string(&disk_nft.nft_keypair);
        let nft_pubkey = nft_keypair.pubkey();
        if !map.contains_key(&set_id) {
            map.insert(set_id, HashMap::new());
        }

        let internal_hashmap_entry = map.entry(set_id);
        internal_hashmap_entry.and_modify(|v| {
            v.insert(
                nft_pubkey,
                NFTInfo {
                    nft_keypair,
                    spl_token_keypair: Keypair::from_base58_string(&disk_nft.spl_token_keypair),
                    spl_token_mint_keypair: Keypair::from_base58_string(
                        &disk_nft.spl_token_mint_keypair,
                    ),
                    is_payer_owner: disk_nft.is_payer_owner,
                    set_id,
                },
            );
        });
    }

    NFTsInfo { nfts: map }
}

fn write_nfts(data_dir: &String, nfts_info: &NFTsInfo) {
    let mut nfts: Vec<NFTInfoDisk> = vec![];

    for (_, nft_map) in &nfts_info.nfts {
        for (_i, nft_info) in nft_map {
            nfts.push(NFTInfoDisk {
                nft_keypair: nft_info.nft_keypair.to_base58_string(),
                spl_token_keypair: nft_info.spl_token_keypair.to_base58_string(),
                spl_token_mint_keypair: nft_info.spl_token_mint_keypair.to_base58_string(),
                is_payer_owner: nft_info.is_payer_owner,
                set_id: nft_info.set_id.to_string(),
            })
        }
    }

    let nfts_info_disk = NFTsInfoDisk { nfts };

    let serialized_data = serde_json::to_string(&nfts_info_disk).unwrap();
    fs::write(&format!("{}/nfts.json", data_dir), serialized_data).unwrap();
}

fn print_available_sets(config: &Config, sets: &NFTSets, nfts: &NFTsInfo, print_nfts: bool) {
    if sets.set_keys.is_empty() {
        println!("No Available sets")
    } else {
        for (i, set_key) in sets.set_keys.iter().enumerate() {
            let set_pubkey = set_key.pubkey();
            let data = config.client.get_account_data(&set_pubkey).unwrap();
            let unpacked_nft_set = PlayerNFTSet::unpack(data.as_slice()).unwrap();
            println!(
                "{}. {} ChainData: name_bin:{:?}",
                i,
                set_key.pubkey(),
                unpacked_nft_set.friendly_name
            );
            if print_nfts {
                print_available_nfts_for_set(&config, set_key.pubkey(), nfts, true);
            }
        }
    }
}

fn print_available_nfts_for_set(
    config: &Config,
    set_id: Pubkey,
    nfts_info: &NFTsInfo,
    add_tabs: bool,
) {
    if !nfts_info.nfts.contains_key(&set_id) {
        println!("No NFTs are available for this set id");
    } else {
        let nfts = &nfts_info.nfts[&set_id];
        for (i, (k, _)) in nfts.iter().enumerate() {
            let data = config.client.get_account_data(k).unwrap();
            let unpacked_nft_state = PlayerNFTState::unpack(data.as_slice()).unwrap();
            if add_tabs {
                println!("===> NFT:{}. {}", i, k);
                print_nft(&unpacked_nft_state);
            } else {
                println!("{}. {}", i, k);
            }
        }
    }
}

fn create_new_set(config: &mut Config, sets: &mut NFTSets) {
    println!("Please enter Friendly name for Set:");
    let mut friendly_name = String::new();
    let mut size = std::io::stdin().read_line(&mut friendly_name).unwrap();
    friendly_name = friendly_name.trim().to_string();
    while size > 16 {
        friendly_name.clear();
        println!("Excessive long friendly name. Please truncate");
        size = std::io::stdin().read_line(&mut friendly_name).unwrap();
        friendly_name = friendly_name.trim().to_string();
    }

    let mut friendly_name_bytes = friendly_name.into_bytes();
    for _i in 0..(16 - friendly_name_bytes.len()) {
        friendly_name_bytes.push(0);
    }

    let new_nft_set_params = NewNFTSetParams {
        friendly_name: friendly_name_bytes.try_into().unwrap(),
        token_program: config.token_program_pubkey,
        mint_authority: COption::None,
    };

    let keypair = new_nft_set_tx(
        &mut config.client,
        new_nft_set_params,
        &config.payer,
        config.nft_program_pubkey,
        &config.admin,
    );
    println!("Set created at: {}", keypair.pubkey());
    sets.set_keys.push(keypair);
}

fn get_nft_number(field_name: &str, optional: bool) -> COption<u32> {
    if optional {
        println!("Please enter {} (To skip press enter)", field_name);
    } else {
        println!("Please enter {}", field_name);
    }

    let mut str = String::new();
    std::io::stdin().read_line(&mut str).unwrap();
    str = str.trim().to_string();

    while u32::from_str(&str).is_err() {
        if optional && str.len() == 0 {
            break;
        }
        println!("Unable to parse: {} into number", str);
        str.clear();
        std::io::stdin().read_line(&mut str).unwrap();
        str = str.trim().to_string();
    }

    if optional && str.len() == 0 {
        COption::None
    } else {
        COption::Some(u32::from_str(&str).unwrap())
    }
}

fn get_nft_string(max_size: usize, field_name: &str, optional: bool) -> COption<Vec<u8>> {
    if optional {
        println!(
            "Please enter {} (Max size: {}) (To skip press enter)",
            field_name, max_size
        );
    } else {
        println!("Please enter {} (Max size: {})", field_name, max_size);
    }

    let mut str = String::new();
    let mut size = std::io::stdin().read_line(&mut str).unwrap();
    str = str.trim().to_string();

    while size > max_size {
        str.clear();
        println!("Excessive long string. Max size: {} is allowed", size);
        size = std::io::stdin().read_line(&mut str).unwrap() - 1;
        str = str.trim().to_string();
    }

    if optional && str.len() == 0 {
        return COption::None;
    }

    let mut str_bytes = str.into_bytes();
    for _i in 0..(max_size - str_bytes.len()) {
        str_bytes.push(0);
    }

    COption::Some(str_bytes)
}

fn convert_nft_field_to_string(data: Vec<u8>) -> String {
    let non_empty_data: Vec<Vec<u8>> = data
        .splitn(1, |o| *o == 0)
        .map(|elem| elem.to_vec())
        .collect();

    String::from_utf8(non_empty_data[0].clone()).unwrap()
}

fn print_nft(nft: &PlayerNFTState) {
    println!(
        "=======> Name: {}",
        convert_nft_field_to_string(nft.name.to_vec())
    );
    println!(
        "=======> Country: {}",
        convert_nft_field_to_string(nft.country.to_vec())
    );
    println!(
        "=======> Club: {}",
        convert_nft_field_to_string(nft.club.to_vec())
    );
    println!("=======> Position: {}", nft.position);
    println!(
        "=======> Jersey: {}",
        convert_nft_field_to_string(nft.jersey.to_vec())
    );
    println!("=======> Season year: {}", nft.season_year);
    println!("=======> Score: {}", nft.score);
    println!("=======> Cost to game: {}", nft.cost_to_game);
    println!(
        "=======> Image: {}",
        convert_nft_field_to_string(nft.image_url.to_vec())
    );
    println!("=======> League rank: {}", nft.league_rank);
}

fn create_new_nft(config: &mut Config, sets: &NFTSets, nfts: &mut NFTsInfo) {
    println!("Please select one set");
    print_available_sets(&config, sets, &nfts, false);

    let mut set_pubkey_str = String::new();
    std::io::stdin().read_line(&mut set_pubkey_str).unwrap();
    set_pubkey_str = set_pubkey_str.trim().to_string();
    while let Err(_e) = Pubkey::from_str(&set_pubkey_str) {
        set_pubkey_str.clear();
        println!("Invalid Pubkey format. Please enter valid pubkey.");
        std::io::stdin().read_line(&mut set_pubkey_str).unwrap();
        set_pubkey_str = set_pubkey_str.trim().to_string();
    }
    let set_pubkey = Pubkey::from_str(&set_pubkey_str).unwrap();

    let name_bytes = get_nft_string(16, "Name", false).unwrap();
    let country_bytes = get_nft_string(16, "Country", false).unwrap();
    let club_bytes = get_nft_string(16, "Club", false).unwrap();
    let position = get_nft_number("Position", false).unwrap();
    let jersey_bytes = get_nft_string(16, "Jersey", false).unwrap();
    let season_year = get_nft_number("Season year", false).unwrap();
    let score = get_nft_number("Score", false).unwrap();
    let cost_to_game = get_nft_number("Cost to game", false).unwrap();
    let image_bytes = get_nft_string(112, "ImageUrl", false).unwrap();
    let league_rank = get_nft_number("League rank", false).unwrap();

    let params = MintNewNFTParams {
        name: name_bytes.try_into().unwrap(),
        country: country_bytes.try_into().unwrap(),
        club: club_bytes.try_into().unwrap(),
        position,
        jersey: jersey_bytes.try_into().unwrap(),
        season_year: season_year as u16,
        score: score as u64,
        cost_to_game,
        image: image_bytes.try_into().unwrap(),
        league_rank,
    };

    let (nft_keypair, spl_token_keypair, spl_token_mint_keypair) = mint_new_nft_tx(
        &mut config.client,
        &config.payer.pubkey(),
        &set_pubkey,
        &config.token_program_pubkey,
        &config.nft_program_pubkey,
        &config.payer,
        params,
    );

    let nft_pubkey = nft_keypair.pubkey();
    let nft_info = NFTInfo {
        nft_keypair,
        spl_token_keypair,
        spl_token_mint_keypair,
        is_payer_owner: true,
        set_id: set_pubkey,
    };

    let internal_hashmap = nfts.nfts.entry(set_pubkey).or_insert(HashMap::new());
    internal_hashmap.insert(nft_pubkey, nft_info);
}

fn do_change_nft(config: &mut Config, sets: &NFTSets, nfts: &mut NFTsInfo) {
    println!("Please select one set");
    print_available_sets(&config, sets, &nfts, false);

    let mut set_pubkey_str = String::new();
    std::io::stdin().read_line(&mut set_pubkey_str).unwrap();
    set_pubkey_str = set_pubkey_str.trim().to_string();
    while let Err(_e) = Pubkey::from_str(&set_pubkey_str) {
        set_pubkey_str.clear();
        println!("Invalid Pubkey format. Please enter valid pubkey.");
        std::io::stdin().read_line(&mut set_pubkey_str).unwrap();
        set_pubkey_str = set_pubkey_str.trim().to_string();
    }
    let set_pubkey = Pubkey::from_str(&set_pubkey_str).unwrap();

    println!("Please select an NFT to change");
    print_available_nfts_for_set(&config, set_pubkey, nfts, false);

    let mut nft_pubkey_str = String::new();
    std::io::stdin().read_line(&mut nft_pubkey_str).unwrap();
    nft_pubkey_str = nft_pubkey_str.trim().to_string();
    while let Err(_e) = Pubkey::from_str(&nft_pubkey_str) {
        nft_pubkey_str.clear();
        println!("Invalid Pubkey format. Please enter valid pubkey.");
        std::io::stdin().read_line(&mut nft_pubkey_str).unwrap();
        nft_pubkey_str = nft_pubkey_str.trim().to_string();
    }
    let nft_pubkey = Pubkey::from_str(&nft_pubkey_str).unwrap();

    let name_bytes = get_nft_string(16, "Name", true);
    let country_bytes = get_nft_string(16, "Country", true);
    let club_bytes = get_nft_string(16, "Club", true);
    let position = get_nft_number("Position", true);
    let jersey_bytes = get_nft_string(16, "Jersey", true);
    let season_year = get_nft_number("Season year", true);
    let score = get_nft_number("Score", true);
    let cost_to_game = get_nft_number("Cost to game", true);
    let image_bytes = get_nft_string(112, "ImageUrl", true);
    let league_rank = get_nft_number("League rank", true);

    let params = ChangeNFTParams {
        name: name_bytes.map(|o| o.try_into().unwrap()),
        country: country_bytes.map(|o| o.try_into().unwrap()),
        club: club_bytes.map(|o| o.try_into().unwrap()),
        position,
        jersey: jersey_bytes.map(|o| o.try_into().unwrap()),
        season_year: season_year.map(|o| o as u16),
        score: score.map(|o| o as u64),
        cost_to_game,
        image: image_bytes.map(|o| o.try_into().unwrap()),
        league_rank,
    };

    change_nft_parameters_tx(
        &mut config.client,
        &config.payer,
        &config.admin,
        &set_pubkey,
        &nft_pubkey,
        &config.nft_program_pubkey,
        params,
    );
    println!("NFT changed");
}

fn do_transfer_nft(config: &mut Config, sets: &NFTSets, nfts: &mut NFTsInfo) {
    println!("Please select one set");
    print_available_sets(&config, sets, &nfts, false);

    let mut set_pubkey_str = String::new();
    std::io::stdin().read_line(&mut set_pubkey_str).unwrap();
    set_pubkey_str = set_pubkey_str.trim().to_string();
    while let Err(_e) = Pubkey::from_str(&set_pubkey_str) {
        set_pubkey_str.clear();
        println!("Invalid Pubkey format. Please enter valid pubkey.");
        std::io::stdin().read_line(&mut set_pubkey_str).unwrap();
        set_pubkey_str = set_pubkey_str.trim().to_string();
    }
    let set_pubkey = Pubkey::from_str(&set_pubkey_str).unwrap();

    println!("Please select an NFT for transfer");
    print_available_nfts_for_set(&config, set_pubkey, nfts, false);

    let mut nft_pubkey_str = String::new();
    std::io::stdin().read_line(&mut nft_pubkey_str).unwrap();
    nft_pubkey_str = nft_pubkey_str.trim().to_string();
    while let Err(_e) = Pubkey::from_str(&nft_pubkey_str) {
        nft_pubkey_str.clear();
        println!("Invalid Pubkey format. Please enter valid pubkey.");
        std::io::stdin().read_line(&mut nft_pubkey_str).unwrap();
        nft_pubkey_str = nft_pubkey_str.trim().to_string();
    }
    let nft_pubkey = Pubkey::from_str(&nft_pubkey_str).unwrap();

    println!("Please paste new owner's pubkey");
    let mut new_owner_pubkey_str = String::new();
    std::io::stdin()
        .read_line(&mut new_owner_pubkey_str)
        .unwrap();
    new_owner_pubkey_str = new_owner_pubkey_str.trim().to_string();
    while let Err(_e) = Pubkey::from_str(&new_owner_pubkey_str) {
        new_owner_pubkey_str.clear();
        println!("Invalid Pubkey format. Please enter valid pubkey.");
        std::io::stdin()
            .read_line(&mut new_owner_pubkey_str)
            .unwrap();
        new_owner_pubkey_str = new_owner_pubkey_str.trim().to_string();
    }
    let new_owner_pubkey = Pubkey::from_str(&new_owner_pubkey_str).unwrap();

    let internal_hashmap_entry = nfts.nfts.entry(set_pubkey);
    internal_hashmap_entry.and_modify(|v| {
        v.entry(nft_pubkey).and_modify(|v| {
            if !v.is_payer_owner {
                println!("We cannot transfer NFT that we do not own.");
                return;
            }
            let dest_account = transfer_nft_tx(
                &mut config.client,
                &config.payer,
                &config.payer,
                &new_owner_pubkey,
                &v.spl_token_keypair.pubkey(),
                &v.spl_token_mint_keypair.pubkey(),
                &set_pubkey,
                &v.nft_keypair.pubkey(),
                &config.token_program_pubkey,
                &config.nft_program_pubkey,
            );
            println!("Transfer Done and new owner is: {}", new_owner_pubkey);
            v.is_payer_owner = false;
            v.spl_token_keypair = dest_account;
        });
    });
}

pub fn main() {
    let args: Vec<String> = env::args().collect();

    let data_dir = if args.len() < 2 {
        println!("No data directory argument passed, using default one.");
        DEFAULT_DATA_PATH.to_string()
    } else {
        println!("Using data directory: {}", args[1]);
        args[1].clone()
    };

    let mut config = read_config(&data_dir);
    let mut sets = read_sets(&data_dir);
    let mut nfts = read_nfts(&data_dir);

    println!(
        "Please ensure sufficient fund in payer key: {}",
        config.payer.pubkey()
    );

    loop {
        println!("Demo choice:");
        println!("1. Create New Set");
        println!("2. Create New NFT");
        println!("3. Transfer NFT");
        println!("4. Change NFT");
        println!("5. Get information about the set");
        println!("6. Quit");

        let mut input = String::new();
        std::io::stdin().read_line(&mut input).unwrap();
        input = input.trim_end().to_string();

        if let Ok(choice) = u8::from_str(input.as_str()) {
            match choice {
                1 => {
                    create_new_set(&mut config, &mut sets);
                    write_sets(&data_dir, &sets)
                }
                2 => {
                    create_new_nft(&mut config, &sets, &mut nfts);
                    write_nfts(&data_dir, &nfts)
                }
                3 => {
                    do_transfer_nft(&mut config, &sets, &mut nfts);
                    write_nfts(&data_dir, &nfts)
                }
                4 => {
                    do_change_nft(&mut config, &sets, &mut nfts);
                }
                5 => print_available_sets(&config, &sets, &nfts, true),
                6 => break,
                _ => println!("Invalid choice"),
            }
        } else {
            println!("Invalid Choice");
        }
    }
}

pub fn change_nft_parameters_tx(
    client: &mut RpcClient,
    payer: &Keypair,
    set_admin: &Keypair,
    nft_set: &Pubkey,
    nft: &Pubkey,
    nft_program: &Pubkey,
    params: ChangeNFTParams,
) {
    let mut transaction = Transaction::new_with_payer(
        &[
            change_nft_parameters(&nft_program, params, &set_admin.pubkey(), &nft_set, &nft)
                .unwrap(),
        ],
        Some(&payer.pubkey()),
    );

    let recent_blockhash = client.get_recent_blockhash().unwrap().0;
    transaction.sign(&[payer, &set_admin].to_vec(), recent_blockhash);
    client
        .send_and_confirm_transaction_with_spinner(&transaction)
        .unwrap();
}

pub fn transfer_nft_tx(
    client: &mut RpcClient,
    payer: &Keypair,
    nft_owner: &Keypair,
    new_owner: &Pubkey,
    source_spl_account: &Pubkey,
    mint_pubkey: &Pubkey,
    nft_set: &Pubkey,
    nft_state: &Pubkey,
    token_program: &Pubkey,
    nft_program: &Pubkey,
) -> Keypair {
    let spl_dest_account = Keypair::new();

    let mut transaction = Transaction::new_with_payer(
        &[
            create_account(
                &payer.pubkey(),
                &spl_dest_account.pubkey(),
                client
                    .get_minimum_balance_for_rent_exemption(
                        spl_token::state::Account::get_packed_len(),
                    )
                    .unwrap(),
                spl_token::state::Account::get_packed_len() as u64,
                &token_program,
            ),
            spl_token::instruction::initialize_account(
                &token_program,
                &spl_dest_account.pubkey(),
                &mint_pubkey,
                &new_owner,
            )
            .unwrap(),
            transfer_nft(
                &nft_program,
                &nft_owner.pubkey(),
                &source_spl_account,
                &spl_dest_account.pubkey(),
                &nft_set,
                &nft_state,
                &token_program,
            )
            .unwrap(),
        ],
        Some(&payer.pubkey()),
    );

    let recent_blockhash = client.get_recent_blockhash().unwrap().0;
    transaction.sign(
        &[payer, &spl_dest_account, &nft_owner].to_vec(),
        recent_blockhash,
    );
    client
        .send_and_confirm_transaction_with_spinner(&transaction)
        .unwrap();

    spl_dest_account
}

pub fn mint_new_nft_tx(
    client: &mut RpcClient,
    user_account: &Pubkey,
    set_pubkey: &Pubkey,
    token_program: &Pubkey,
    nft_program: &Pubkey,
    payer: &Keypair,
    params: MintNewNFTParams,
) -> (Keypair, Keypair, Keypair) {
    let spl_token_mint_keypair = Keypair::new();
    let spl_token_keypair = Keypair::new();
    let nft_keypair = Keypair::new();
    let spl_token_mint_authority = Keypair::new();

    let mut transaction = Transaction::new_with_payer(
        &[
            create_account(
                &payer.pubkey(),
                &nft_keypair.pubkey(),
                client
                    .get_minimum_balance_for_rent_exemption(PlayerNFTState::get_packed_len())
                    .unwrap(),
                PlayerNFTState::get_packed_len() as u64,
                &nft_program,
            ),
            create_account(
                &payer.pubkey(),
                &spl_token_mint_keypair.pubkey(),
                client
                    .get_minimum_balance_for_rent_exemption(Mint::get_packed_len())
                    .unwrap(),
                Mint::get_packed_len() as u64,
                &token_program,
            ),
            create_account(
                &payer.pubkey(),
                &spl_token_keypair.pubkey(),
                client
                    .get_minimum_balance_for_rent_exemption(
                        spl_token::state::Account::get_packed_len(),
                    )
                    .unwrap(),
                spl_token::state::Account::get_packed_len() as u64,
                &token_program,
            ),
            mint_new_nft(
                &nft_program,
                params,
                user_account,
                set_pubkey,
                token_program,
                &spl_token_mint_keypair.pubkey(),
                &spl_token_keypair.pubkey(),
                &nft_keypair.pubkey(),
                &spl_token_mint_authority.pubkey(),
                None,
            )
            .unwrap(),
        ],
        Some(&payer.pubkey()),
    );

    let recent_blockhash = client.get_recent_blockhash().unwrap().0;
    transaction.sign(
        &[
            payer,
            &spl_token_mint_authority,
            &spl_token_keypair,
            &spl_token_mint_keypair,
            &nft_keypair,
        ]
        .to_vec(),
        recent_blockhash,
    );
    client
        .send_and_confirm_transaction_with_spinner(&transaction)
        .unwrap();

    (nft_keypair, spl_token_keypair, spl_token_mint_keypair)
}

pub fn new_nft_set_tx(
    client: &mut RpcClient,
    params: NewNFTSetParams,
    payer: &Keypair,
    nft_program: Pubkey,
    admin: &Keypair,
) -> Keypair {
    let new_set_keypair = Keypair::new();
    let new_set_pubkey = new_set_keypair.pubkey();

    let mut transaction = Transaction::new_with_payer(
        &[
            create_account(
                &payer.pubkey(),
                &new_set_pubkey,
                client
                    .get_minimum_balance_for_rent_exemption(PlayerNFTSet::get_packed_len())
                    .unwrap(),
                PlayerNFTSet::get_packed_len() as u64,
                &nft_program,
            ),
            new_nft_set(&nft_program, params, &admin.pubkey(), &new_set_pubkey).unwrap(),
        ],
        Some(&payer.pubkey()),
    );

    let recent_blockhash = client.get_recent_blockhash().unwrap().0;
    transaction.sign(&[payer, &new_set_keypair], recent_blockhash);
    client
        .send_and_confirm_transaction_with_spinner(&transaction)
        .unwrap();

    new_set_keypair
}

pub fn change_nft_set_administrator(
    client: &mut RpcClient,
    payer: &Keypair,
    set_account: &Pubkey,
    admin: &Keypair,
    new_admin: &Pubkey,
    nft_program_pubkey: &Pubkey,
) {
    let admin_change_params = ChangeNFTSetAdminParams {
        new_admin: *new_admin,
    };

    let mut transaction = Transaction::new_with_payer(
        &[change_nft_set_admin(
            &nft_program_pubkey,
            admin_change_params,
            &admin.pubkey(),
            set_account,
        )
        .unwrap()],
        Some(&payer.pubkey()),
    );

    let recent_blockhash = client.get_recent_blockhash().unwrap().0;
    transaction.sign(&[payer, &admin], recent_blockhash);
    client
        .send_and_confirm_transaction_with_spinner(&transaction)
        .unwrap();
}
