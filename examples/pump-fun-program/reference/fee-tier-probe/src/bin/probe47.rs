//! Isolates and confirms `pump_amm::init_boost`'s own real behavior directly
//! (probe45/probe46 only ever observed it as a nested CPI inside a full
//! `migrate_v2` call, so its `creator`-account constraint and its own
//! emitted `InitBoostEvent` were never independently verified) -- calls it
//! standalone against a hand-fixtured `Pool` account, bypassing `migrate_v2`
//! and `create_pool` entirely. Also verifies `toggle_boost` and
//! `set_boost_authority`, which so far have only been read from the IDL,
//! never executed against real bytecode.
//!
//! Usage: cargo run --bin probe47 [-- <rpc-url>]

use litesvm::LiteSVM;
use solana_rpc_client::rpc_client::RpcClient;
use solana_sdk::{
    account::Account,
    clock::Clock,
    instruction::{AccountMeta, Instruction},
    message::Message,
    pubkey::Pubkey,
    signature::{Keypair, Signer},
    transaction::Transaction,
};
use std::str::FromStr;
use std::time::Duration;

const PUMP_AMM_PROGRAM_ID: &str = "pAMMBay6oceH9fJKBRHGP5D4bD4sWpmSwMn52FMfXEA";
const ASSOCIATED_TOKEN_PROGRAM_ID: &str = "ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL";
const SYSTEM_PROGRAM_ID: &str = "11111111111111111111111111111111";
const TOKEN_PROGRAM_ID: &str = "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA";
const USDC_MINT: &str = "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v";

const POOL_DISCRIMINATOR: [u8; 8] = [241, 154, 109, 4, 17, 177, 109, 188];
const INIT_BOOST_DISCRIMINATOR: [u8; 8] = [140, 233, 33, 94, 132, 90, 194, 143];
const TOGGLE_BOOST_DISCRIMINATOR: [u8; 8] = [117, 161, 160, 74, 223, 137, 118, 99];
const SET_BOOST_AUTHORITY_DISCRIMINATOR: [u8; 8] = [227, 149, 76, 42, 130, 39, 234, 205];

fn pk(s: &str) -> Pubkey {
    Pubkey::from_str(s).unwrap()
}

fn set_compute_unit_limit_ix(units: u32) -> Instruction {
    let mut data = vec![2u8];
    data.extend_from_slice(&units.to_le_bytes());
    Instruction { program_id: pk("ComputeBudget111111111111111111111111111111"), accounts: vec![], data }
}

fn ata_address(owner: &Pubkey, mint: &Pubkey, token_program: &Pubkey) -> Pubkey {
    Pubkey::find_program_address(&[owner.as_ref(), token_program.as_ref(), mint.as_ref()], &pk(ASSOCIATED_TOKEN_PROGRAM_ID)).0
}

fn mint_account_data(decimals: u8) -> Vec<u8> {
    let mut data = vec![0u8; 82];
    data[44] = decimals;
    data[45] = 1;
    data
}

fn token_account_data(mint: &Pubkey, owner: &Pubkey, amount: u64) -> Vec<u8> {
    let mut data = vec![0u8; 165];
    data[0..32].copy_from_slice(mint.as_ref());
    data[32..64].copy_from_slice(owner.as_ref());
    data[64..72].copy_from_slice(&amount.to_le_bytes());
    data[108] = 1;
    data
}

fn pool_account_data(
    pool_bump: u8,
    index: u16,
    creator: &Pubkey,
    base_mint: &Pubkey,
    quote_mint: &Pubkey,
    lp_mint: &Pubkey,
    pool_base_token_account: &Pubkey,
    pool_quote_token_account: &Pubkey,
    coin_creator: &Pubkey,
    lp_supply: u64,
    is_mayhem_mode: bool,
    is_cashback_coin: bool,
    virtual_quote_reserves: i128,
) -> Vec<u8> {
    let mut data = Vec::new();
    data.extend_from_slice(&POOL_DISCRIMINATOR);
    data.push(pool_bump);
    data.extend_from_slice(&index.to_le_bytes());
    data.extend_from_slice(creator.as_ref());
    data.extend_from_slice(base_mint.as_ref());
    data.extend_from_slice(quote_mint.as_ref());
    data.extend_from_slice(lp_mint.as_ref());
    data.extend_from_slice(pool_base_token_account.as_ref());
    data.extend_from_slice(pool_quote_token_account.as_ref());
    data.extend_from_slice(coin_creator.as_ref());
    data.extend_from_slice(&lp_supply.to_le_bytes());
    data.push(if is_mayhem_mode { 1 } else { 0 });
    data.push(if is_cashback_coin { 1 } else { 0 });
    data.extend_from_slice(&virtual_quote_reserves.to_le_bytes());
    assert_eq!(data.len(), 261, "Pool layout mismatch");
    data
}

fn fetch_account(client: &RpcClient, pubkey: &Pubkey) -> Result<Account, String> {
    let remote = client.get_account(pubkey).map_err(|e| format!("{e:?}"))?;
    Ok(Account {
        lamports: remote.lamports,
        data: remote.data.clone(),
        owner: Pubkey::from_str(&remote.owner.to_string()).map_err(|e| format!("{e:?}"))?,
        executable: remote.executable,
        rent_epoch: remote.rent_epoch,
    })
}

fn token_balance(svm: &LiteSVM, addr: &Pubkey) -> Option<u64> {
    svm.get_account(addr).map(|acc| u64::from_le_bytes(acc.data[64..72].try_into().unwrap()))
}

fn print_fail(svm_err_meta_logs: &[String]) {
    for line in svm_err_meta_logs {
        println!("  LOG: {line}");
    }
}

fn main() {
    let rpc_url = std::env::args().nth(1).unwrap_or_else(|| "https://api.mainnet-beta.solana.com".to_string());
    let client = RpcClient::new_with_timeout(rpc_url, Duration::from_secs(60));

    let pump_amm_program = pk(PUMP_AMM_PROGRAM_ID);
    let system_program = pk(SYSTEM_PROGRAM_ID);
    let base_token_program = pk(TOKEN_PROGRAM_ID);
    let quote_token_program = pk(TOKEN_PROGRAM_ID);
    let associated_token_program = pk(ASSOCIATED_TOKEN_PROGRAM_ID);
    let quote_mint = pk(USDC_MINT);

    let mut svm = LiteSVM::new();
    let so_path = format!("{}/../pump-rust-client/artifacts/pump_amm.so", env!("CARGO_MANIFEST_DIR"));
    svm.add_program_from_file(pump_amm_program, &so_path).unwrap_or_else(|e| panic!("load pump_amm.so: {e:?}"));
    svm.set_sysvar::<Clock>(&Clock { unix_timestamp: 1_700_000_000, slot: 100, epoch: 0, leader_schedule_epoch: 0, epoch_start_timestamp: 0 });

    let (amm_global_config, _) = Pubkey::find_program_address(&[b"global_config"], &pump_amm_program);
    let mut amm_global_config_account = fetch_account(&client, &amm_global_config).expect("fetch amm global_config failed");
    let real_admin = Pubkey::try_from(&amm_global_config_account.data[8..40]).unwrap();
    println!("real amm_global_config.admin = {real_admin}");

    // Patch admin (offset 8..40) and boost_authority (offset 907..939) to
    // keypairs we control, same technique already validated in probe46.rs --
    // this only affects our own in-memory litesvm copy.
    let admin_kp = Keypair::new();
    amm_global_config_account.data[8..40].copy_from_slice(admin_kp.pubkey().as_ref());
    svm.set_account(amm_global_config, amm_global_config_account.clone()).unwrap();

    let quote_mint_account = fetch_account(&client, &quote_mint).expect("fetch USDC mint failed");
    svm.set_account(quote_mint, quote_mint_account).unwrap();

    let base_mint = Pubkey::new_unique();
    svm.set_account(base_mint, Account { lamports: 10_000_000, data: mint_account_data(6), owner: base_token_program, executable: false, rent_epoch: 0 }).unwrap();
    let pool_creator_field = Pubkey::new_unique(); // Pool's own stored `creator` field
    // index deliberately 0 this time (previous run used 7, no effect
    // observed there, but every real migrate_v2-created pool is always at
    // index 0 -- testing whether boost only applies to the canonical
    // migration pool).
    let pool_index: u16 = 0;
    let (pool, pool_bump) = Pubkey::find_program_address(
        &[b"pool", &pool_index.to_le_bytes(), pool_creator_field.as_ref(), base_mint.as_ref(), quote_mint.as_ref()],
        &pump_amm_program,
    );
    let lp_mint = Pubkey::new_unique();
    let pool_base_token_account = ata_address(&pool, &base_mint, &base_token_program);
    let pool_quote_token_account = ata_address(&pool, &quote_mint, &quote_token_program);
    let coin_creator = Pubkey::new_unique();

    let real_quote_split_amount = 3_460_000_000u64; // mirrors probe45/46's real observed boost allocation

    // lp_supply deliberately nonzero this time -- probe47's first run left
    // it at 0 and init_boost silently wrote virtual_quote_reserves = 0 with
    // no error, contradicting a real mainnet InitBoostEvent decode showing
    // it genuinely nonzero (17,584,505,423) for an actual migrated pool.
    // Testing whether init_boost gates its real logic on lp_supply > 0
    // (i.e. only acts on an already-liquid pool).
    let pool_data = pool_account_data(
        pool_bump, pool_index, &pool_creator_field, &base_mint, &quote_mint, &lp_mint,
        &pool_base_token_account, &pool_quote_token_account, &coin_creator,
        1_000_000_000, false, false, 0, // virtual_quote_reserves = 0 -- not yet boost-initialized
    );
    svm.set_account(pool, Account { lamports: 10_000_000, data: pool_data, owner: pump_amm_program, executable: false, rent_epoch: 0 }).unwrap();

    svm.set_account(
        pool_base_token_account,
        Account { lamports: 2_039_280, data: token_account_data(&base_mint, &pool, 194_382_022_482_831), owner: base_token_program, executable: false, rent_epoch: 0 },
    ).unwrap();
    // pool_quote_token_account holds the FULL pre-split amount this time
    // (matching what create_pool would deposit before any boost split, not
    // the already-split 80% figure used in the previous attempt) --
    // probe46's real migrate_v2 log showed init_boost itself creating
    // boost_vault fresh (a plain `Create`, not `CreateIdempotent`)
    // immediately after "Instruction: InitBoost" began, suggesting
    // init_boost -- not migrate_v2 -- performs the split, funded by moving
    // quote OUT of pool_quote_token_account into the newly-created
    // boost_vault. Testing that directly.
    svm.set_account(
        pool_quote_token_account,
        Account { lamports: 2_039_280, data: token_account_data(&quote_mint, &pool, real_quote_split_amount * 5), owner: quote_token_program, executable: false, rent_epoch: 0 },
    ).unwrap();

    let (boost_vault_authority, _) = Pubkey::find_program_address(&[b"boost_vault", pool.as_ref()], &pump_amm_program);
    let boost_vault = ata_address(&boost_vault_authority, &quote_mint, &quote_token_program);
    // boost_vault deliberately left NOT pre-created this time -- letting
    // init_boost create it itself, per the hypothesis above.

    let (event_authority, _) = Pubkey::find_program_address(&[b"__event_authority"], &pump_amm_program);

    // "creator" signer deliberately does NOT match pool_creator_field (the
    // Pool account's own stored `creator` field) -- testing whether the real
    // program enforces a relational check here or accepts any signer.
    let creator_signer = Keypair::new();
    svm.airdrop(&creator_signer.pubkey(), 10_000_000_000).unwrap();
    println!("pool.creator (stored field) = {pool_creator_field}");
    println!("init_boost's `creator` account (signer, deliberately different) = {}", creator_signer.pubkey());

    let init_boost_ix = Instruction {
        program_id: pump_amm_program,
        accounts: vec![
            AccountMeta::new(pool, false),
            AccountMeta::new_readonly(amm_global_config, false),
            AccountMeta::new(creator_signer.pubkey(), true),
            AccountMeta::new_readonly(base_mint, false),
            AccountMeta::new_readonly(quote_mint, false),
            AccountMeta::new_readonly(pool_base_token_account, false),
            AccountMeta::new(pool_quote_token_account, false),
            AccountMeta::new_readonly(boost_vault_authority, false),
            AccountMeta::new(boost_vault, false),
            AccountMeta::new_readonly(quote_token_program, false),
            AccountMeta::new_readonly(system_program, false),
            AccountMeta::new_readonly(associated_token_program, false),
            AccountMeta::new_readonly(event_authority, false),
            AccountMeta::new_readonly(pump_amm_program, false),
        ],
        data: INIT_BOOST_DISCRIMINATOR.to_vec(),
    };

    println!("\n=== Test 1: init_boost (standalone, creator != pool.creator, boost_vault not pre-created, pool_quote_token_account holds full pre-split amount) ===");
    println!("pool_quote_token_account before = {:?}", token_balance(&svm, &pool_quote_token_account));
    println!("boost_vault before = {:?} (expect None -- not yet created)", token_balance(&svm, &boost_vault));
    let blockhash = svm.latest_blockhash();
    let msg = Message::new(&[set_compute_unit_limit_ix(400_000), init_boost_ix], Some(&creator_signer.pubkey()));
    let account_keys: Vec<Pubkey> = msg.account_keys.clone();
    let tx = Transaction::new(&[&creator_signer], msg, blockhash);
    match svm.send_transaction(tx) {
        Ok(meta) => {
            println!("RESULT: SUCCESS, {} CU", meta.compute_units_consumed);
            if let Some(acc) = svm.get_account(&pool) {
                let vqr = i128::from_le_bytes(acc.data[245..261].try_into().unwrap());
                println!("pool.virtual_quote_reserves after = {vqr}");
            }
            println!("boost_vault after = {:?}", token_balance(&svm, &boost_vault));
            println!("pool_quote_token_account after = {:?}", token_balance(&svm, &pool_quote_token_account));
            println!("pool_base_token_account after = {:?} (unchanged expected)", token_balance(&svm, &pool_base_token_account));
            println!("-- scanning structured inner_instructions for the InitBoostEvent (avoids log truncation) --");
            for top_level in &meta.inner_instructions {
                for inner in top_level {
                    let prog_idx = inner.instruction.program_id_index as usize;
                    let Some(prog) = account_keys.get(prog_idx) else { continue };
                    if *prog != pump_amm_program || inner.instruction.accounts.len() != 1 {
                        continue;
                    }
                    let data = &inner.instruction.data;
                    if data.len() >= 16 + 8 + 32 + 32 + 32 + 24 {
                        let off = 16 + 8 + 32 + 32 + 32;
                        let vqr = i128::from_le_bytes(data[off..off + 16].try_into().unwrap());
                        let real_after = u64::from_le_bytes(data[off + 16..off + 24].try_into().unwrap());
                        println!("InitBoostEvent: virtual_quote_reserves={vqr} real_quote_reserves_after={real_after}");
                    }
                }
            }
            println!("logs:");
            for l in &meta.logs {
                println!("  {l}");
            }
        }
        Err(e) => {
            println!("RESULT: FAILED: {:?}", e.err);
            print_fail(&e.meta.logs);
        }
    }

    // ---- Test 2: toggle_boost ----
    println!("\n=== Test 2: toggle_boost(enabled=false) ===");
    let mut toggle_data = TOGGLE_BOOST_DISCRIMINATOR.to_vec();
    toggle_data.push(0u8); // enabled = false
    let toggle_ix = Instruction {
        program_id: pump_amm_program,
        accounts: vec![
            AccountMeta::new_readonly(admin_kp.pubkey(), true),
            AccountMeta::new(amm_global_config, false),
        ],
        data: toggle_data,
    };
    let blockhash2 = svm.latest_blockhash();
    let msg2 = Message::new(&[set_compute_unit_limit_ix(100_000), toggle_ix], Some(&admin_kp.pubkey()));
    svm.airdrop(&admin_kp.pubkey(), 10_000_000_000).unwrap();
    let tx2 = Transaction::new(&[&admin_kp], msg2, blockhash2);
    match svm.send_transaction(tx2) {
        Ok(meta) => {
            println!("RESULT: SUCCESS, {} CU", meta.compute_units_consumed);
            if let Some(acc) = svm.get_account(&amm_global_config) {
                println!("global_config.boost_enabled after = {}", acc.data[939]);
            }
        }
        Err(e) => {
            println!("RESULT: FAILED: {:?}", e.err);
            print_fail(&e.meta.logs);
        }
    }

    // ---- Test 3: set_boost_authority ----
    println!("\n=== Test 3: set_boost_authority ===");
    let new_boost_authority = Pubkey::new_unique();
    let set_ba_ix = Instruction {
        program_id: pump_amm_program,
        accounts: vec![
            AccountMeta::new_readonly(admin_kp.pubkey(), true),
            AccountMeta::new(amm_global_config, false),
            AccountMeta::new_readonly(new_boost_authority, false),
            AccountMeta::new_readonly(system_program, false),
            AccountMeta::new_readonly(event_authority, false),
            AccountMeta::new_readonly(pump_amm_program, false),
        ],
        data: SET_BOOST_AUTHORITY_DISCRIMINATOR.to_vec(),
    };
    let blockhash3 = svm.latest_blockhash();
    let msg3 = Message::new(&[set_compute_unit_limit_ix(100_000), set_ba_ix], Some(&admin_kp.pubkey()));
    let tx3 = Transaction::new(&[&admin_kp], msg3, blockhash3);
    match svm.send_transaction(tx3) {
        Ok(meta) => {
            println!("RESULT: SUCCESS, {} CU", meta.compute_units_consumed);
            if let Some(acc) = svm.get_account(&amm_global_config) {
                let ba = Pubkey::try_from(&acc.data[907..939]).unwrap();
                println!("global_config.boost_authority after = {ba} (expected {new_boost_authority})");
            }
        }
        Err(e) => {
            println!("RESULT: FAILED: {:?}", e.err);
            print_fail(&e.meta.logs);
        }
    }

    let _ = token_balance(&svm, &boost_vault);
}
