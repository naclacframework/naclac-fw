//! Follow-up to `probe57.rs` through `probe62.rs`, which DISPROVED six
//! candidates: each of `bonding_curve`'s four reserve fields alone,
//! `associated_bonding_curve`'s real token balance alone, and
//! `real_token_reserves == 0 && real_quote_reserves == 0` together — every
//! one let the real program run its full CPI sequence.
//!
//! Realization: `probe56`'s original "second call no-ops" observation
//! was never actually a clean isolation either. By the time that second
//! call ran, TWO things were simultaneously true: all four `bonding_curve`
//! reserve fields were 0 (from the real first migration), AND `pool`/
//! `lp_mint`/etc. already existed (also from that same first migration).
//! Nothing in `probe56` isolated which of those two conditions is the
//! actual trigger.
//!
//! This probe finally isolates it: ALL FOUR reserve fields hand-set to 0
//! directly (matching the real "already migrated" state byte-for-byte),
//! but `pool`/`lp_mint`/etc. never created — no real migration ever ran.
//! If real `pump.so` still no-ops here, that confirms the check reads
//! `bonding_curve`'s own fields (all four together), not `pool`'s
//! existence. If it instead proceeds and fails downstream, the real check
//! must be based on `pool` (or some other account) already existing.
//!
//! Usage: cargo run --bin probe63

use litesvm::LiteSVM;
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

const PUMP_PROGRAM_ID: &str = "6EF8rrecthR5Dkzon8Nwu78hRvfCKubJ14M5uBEwF6P";
const PUMP_AMM_PROGRAM_ID: &str = "pAMMBay6oceH9fJKBRHGP5D4bD4sWpmSwMn52FMfXEA";
const TOKEN_PROGRAM_ID: &str = "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA";
const TOKEN_2022_PROGRAM_ID: &str = "TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb";
const ASSOCIATED_TOKEN_PROGRAM_ID: &str = "ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL";
const SYSTEM_PROGRAM_ID: &str = "11111111111111111111111111111111";
const RENT_SYSVAR_ID: &str = "SysvarRent111111111111111111111111111111111";
const WSOL_MINT_ID: &str = "So11111111111111111111111111111111111111112";

const MIGRATE_DISCRIMINATOR: [u8; 8] = [155, 234, 231, 146, 236, 158, 162, 30];
const GLOBAL_DISCRIMINATOR: [u8; 8] = [167, 232, 232, 177, 200, 108, 114, 127];
const BONDING_CURVE_DISCRIMINATOR: [u8; 8] = [23, 183, 248, 55, 96, 216, 172, 96];
const GLOBAL_CONFIG_DISCRIMINATOR: [u8; 8] = [149, 8, 156, 202, 160, 252, 176, 217];

const VIRTUAL_TOKEN_RESERVES: u64 = 200_000_000_000_000;
const VIRTUAL_SOL_RESERVES: u64 = 115_005_359_056;
const REAL_TOKEN_RESERVES: u64 = 50_000_000_000_000;
const REAL_SOL_RESERVES: u64 = 85_000_000_000;
const TOKEN_TOTAL_SUPPLY: u64 = 1_000_000_000_000_000;

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

fn mint_account_data(mint_authority: Option<&Pubkey>, decimals: u8) -> Vec<u8> {
    let mut data = vec![0u8; 82];
    if let Some(auth) = mint_authority {
        data[0..4].copy_from_slice(&1u32.to_le_bytes());
        data[4..36].copy_from_slice(auth.as_ref());
    }
    data[36..44].copy_from_slice(&0u64.to_le_bytes());
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

fn global_account_data(withdraw_authority: &Pubkey) -> Vec<u8> {
    let mut data = Vec::new();
    data.extend_from_slice(&GLOBAL_DISCRIMINATOR);
    data.push(1); // initialized
    data.extend_from_slice(&[0u8; 32]); // authority
    data.extend_from_slice(&[0u8; 32]); // fee_recipient
    data.extend_from_slice(&VIRTUAL_TOKEN_RESERVES.to_le_bytes());
    data.extend_from_slice(&VIRTUAL_SOL_RESERVES.to_le_bytes());
    data.extend_from_slice(&REAL_TOKEN_RESERVES.to_le_bytes());
    data.extend_from_slice(&TOKEN_TOTAL_SUPPLY.to_le_bytes());
    data.extend_from_slice(&0u64.to_le_bytes()); // fee_basis_points
    data.extend_from_slice(withdraw_authority.as_ref());
    data.push(1); // enable_migrate = true
    data.extend_from_slice(&[0u8; 8 * 2]);
    data.extend_from_slice(&[0u8; 32 * 7]);
    data.extend_from_slice(&[0u8; 32 * 2]);
    data.push(0);
    data.extend_from_slice(&[0u8; 32]);
    data.extend_from_slice(&[0u8; 32]);
    data.push(0);
    data.extend_from_slice(&[0u8; 32 * 7]);
    data.push(0);
    data.extend_from_slice(&[0u8; 32 * 8]);
    data.extend_from_slice(&0u64.to_le_bytes());
    data.extend_from_slice(&0u64.to_le_bytes());
    data.extend_from_slice(&[0u8; 32]);
    data
}

fn bonding_curve_account_data(creator: &Pubkey, complete: bool, virtual_token_reserves: u64, virtual_sol_reserves: u64, real_token_reserves: u64, real_sol_reserves: u64) -> Vec<u8> {
    let mut data = Vec::new();
    data.extend_from_slice(&BONDING_CURVE_DISCRIMINATOR);
    data.extend_from_slice(&virtual_token_reserves.to_le_bytes());
    data.extend_from_slice(&virtual_sol_reserves.to_le_bytes());
    data.extend_from_slice(&real_token_reserves.to_le_bytes());
    data.extend_from_slice(&real_sol_reserves.to_le_bytes());
    data.extend_from_slice(&TOKEN_TOTAL_SUPPLY.to_le_bytes());
    data.push(complete as u8);
    data.extend_from_slice(creator.as_ref());
    data.push(0);
    data.push(0);
    data.extend_from_slice(&[0u8; 32]);
    data
}

fn global_config_account_data() -> Vec<u8> {
    let mut data = Vec::new();
    data.extend_from_slice(&GLOBAL_CONFIG_DISCRIMINATOR);
    data.extend_from_slice(&[0u8; 32]);
    data.extend_from_slice(&0u64.to_le_bytes());
    data.extend_from_slice(&0u64.to_le_bytes());
    data.push(0);
    data.extend_from_slice(&[0u8; 32 * 8]);
    data.extend_from_slice(&0u64.to_le_bytes());
    data.extend_from_slice(&[0u8; 32]);
    data.extend_from_slice(&[0u8; 32]);
    data.extend_from_slice(&[0u8; 32]);
    data.push(0);
    data.extend_from_slice(&[0u8; 32 * 7]);
    data.push(0);
    data.extend_from_slice(&[0u8; 32 * 8]);
    data.extend_from_slice(&0u64.to_le_bytes());
    data.extend_from_slice(&[0u8; 32]);
    data.push(0);
    data
}

fn run_migrate(svm: &mut LiteSVM, user: &Keypair, ix: &Instruction, label: &str) {
    let blockhash = svm.latest_blockhash();
    let msg = Message::new(&[set_compute_unit_limit_ix(400_000), ix.clone()], Some(&user.pubkey()));
    let account_keys: Vec<Pubkey> = msg.account_keys.clone();
    let tx = Transaction::new(&[user], msg, blockhash);

    println!("\n========== {label} ==========");
    match svm.send_transaction(tx) {
        Ok(meta) => {
            println!("RESULT: SUCCESS, {} CU", meta.compute_units_consumed);
            for line in &meta.logs {
                println!("  LOG: {line}");
            }
            println!("-- inner instructions --");
            for (outer_idx, inner_list) in meta.inner_instructions.iter().enumerate() {
                for inner in inner_list {
                    let ci = &inner.instruction;
                    let program = account_keys.get(ci.program_id_index as usize).map(|p| p.to_string()).unwrap_or_else(|| "?".to_string());
                    println!("  outer[{outer_idx}] program={program} data={:02x?}", ci.data);
                }
            }
            if meta.inner_instructions.iter().all(|v| v.is_empty()) {
                println!("  (zero inner instructions -- real early no-op return, before any CPI)");
            }
        }
        Err(e) => {
            println!("RESULT: FAILED: {:?}", e.err);
            for line in &e.meta.logs {
                println!("  LOG: {line}");
            }
        }
    }
}

fn main() {
    let pump_program = pk(PUMP_PROGRAM_ID);
    let pump_amm_program = pk(PUMP_AMM_PROGRAM_ID);
    let token_program = pk(TOKEN_PROGRAM_ID);
    let token_2022_program = pk(TOKEN_2022_PROGRAM_ID);
    let associated_token_program = pk(ASSOCIATED_TOKEN_PROGRAM_ID);
    let system_program = pk(SYSTEM_PROGRAM_ID);
    let rent_sysvar = pk(RENT_SYSVAR_ID);
    let wsol_mint = pk(WSOL_MINT_ID);

    let mut svm = LiteSVM::new();
    for (program_id, so_name) in [(pump_program, "pump.so"), (pump_amm_program, "pump_amm.so")] {
        let so_path = format!("{}/../pump-rust-client/artifacts/{}", env!("CARGO_MANIFEST_DIR"), so_name);
        svm.add_program_from_file(program_id, &so_path).unwrap_or_else(|e| panic!("load {so_name}: {e:?}"));
    }
    svm.set_sysvar::<Clock>(&Clock { unix_timestamp: 1_700_000_000, slot: 100, epoch: 0, leader_schedule_epoch: 0, epoch_start_timestamp: 0 });

    let user = Keypair::new();
    svm.airdrop(&user.pubkey(), 10_000_000_000).unwrap();

    let withdraw_authority = Keypair::new();
    svm.airdrop(&withdraw_authority.pubkey(), 50_000_000_000).unwrap();

    let creator = Pubkey::new_unique();
    let mint = Pubkey::new_unique();
    svm.set_account(mint, Account { lamports: 10_000_000, data: mint_account_data(None, 6), owner: token_program, executable: false, rent_epoch: 0 }).unwrap();
    svm.set_account(wsol_mint, Account { lamports: 10_000_000, data: mint_account_data(None, 9), owner: token_program, executable: false, rent_epoch: 0 }).unwrap();

    let (global, _) = Pubkey::find_program_address(&[b"global"], &pump_program);
    svm.set_account(global, Account { lamports: 10_000_000, data: global_account_data(&withdraw_authority.pubkey()), owner: pump_program, executable: false, rent_epoch: 0 }).unwrap();

    // Matches the real "already migrated" state byte-for-byte (confirmed
    // against 2 real mainnet bonding curves fetched live via getAccountInfo):
    // all four reserve fields at 0, `complete = true`. `associated_bonding_curve`'s
    // real token balance is also set to 0, matching what a real migration
    // would leave behind. Unlike `probe56`, `pool`/`lp_mint`/etc. below are
    // never created — no real migration CPI ever ran here.
    let (bonding_curve, _) = Pubkey::find_program_address(&[b"bonding-curve", mint.as_ref()], &pump_program);
    svm.set_account(
        bonding_curve,
        Account {
            lamports: 10_000_000 + REAL_SOL_RESERVES,
            data: bonding_curve_account_data(&creator, true, 0, 0, 0, 0),
            owner: pump_program,
            executable: false,
            rent_epoch: 0,
        },
    )
    .unwrap();

    let associated_bonding_curve = ata_address(&bonding_curve, &mint, &token_program);
    svm.set_account(associated_bonding_curve, Account { lamports: 2_039_280, data: token_account_data(&mint, &bonding_curve, 0), owner: token_program, executable: false, rent_epoch: 0 }).unwrap();

    let (amm_global_config, _) = Pubkey::find_program_address(&[b"global_config"], &pump_amm_program);
    svm.set_account(amm_global_config, Account { lamports: 10_000_000, data: global_config_account_data(), owner: pump_amm_program, executable: false, rent_epoch: 0 }).unwrap();

    let (pool_authority, _) = Pubkey::find_program_address(&[b"pool-authority", mint.as_ref()], &pump_program);
    svm.set_account(pool_authority, Account { lamports: 20_000_000_000, data: vec![], owner: system_program, executable: false, rent_epoch: 0 }).unwrap();

    let index: [u8; 2] = [0, 0];
    let (pool, _) = Pubkey::find_program_address(&[b"pool", &index, pool_authority.as_ref(), mint.as_ref(), wsol_mint.as_ref()], &pump_amm_program);
    let (lp_mint, _) = Pubkey::find_program_address(&[b"pool_lp_mint", pool.as_ref()], &pump_amm_program);
    let pool_authority_mint_account = ata_address(&pool_authority, &mint, &token_program);
    let pool_authority_wsol_account = ata_address(&pool_authority, &wsol_mint, &token_program);
    let user_pool_token_account = ata_address(&pool_authority, &lp_mint, &token_2022_program);
    let pool_base_token_account = ata_address(&pool, &mint, &token_program);
    let pool_quote_token_account = ata_address(&pool, &wsol_mint, &token_program);
    let pump_amm_event_authority = Pubkey::find_program_address(&[b"__event_authority"], &pump_amm_program).0;
    let event_authority = Pubkey::find_program_address(&[b"__event_authority"], &pump_program).0;

    let data = MIGRATE_DISCRIMINATOR.to_vec();
    let ix = Instruction {
        program_id: pump_program,
        accounts: vec![
            AccountMeta::new_readonly(global, false),
            AccountMeta::new(withdraw_authority.pubkey(), false),
            AccountMeta::new_readonly(mint, false),
            AccountMeta::new(bonding_curve, false),
            AccountMeta::new(associated_bonding_curve, false),
            AccountMeta::new_readonly(user.pubkey(), true),
            AccountMeta::new_readonly(system_program, false),
            AccountMeta::new_readonly(token_program, false),
            AccountMeta::new_readonly(pump_amm_program, false),
            AccountMeta::new(pool, false),
            AccountMeta::new(pool_authority, false),
            AccountMeta::new(pool_authority_mint_account, false),
            AccountMeta::new(pool_authority_wsol_account, false),
            AccountMeta::new_readonly(amm_global_config, false),
            AccountMeta::new_readonly(wsol_mint, false),
            AccountMeta::new(lp_mint, false),
            AccountMeta::new(user_pool_token_account, false),
            AccountMeta::new(pool_base_token_account, false),
            AccountMeta::new(pool_quote_token_account, false),
            AccountMeta::new_readonly(token_2022_program, false),
            AccountMeta::new_readonly(associated_token_program, false),
            AccountMeta::new_readonly(pump_amm_event_authority, false),
            AccountMeta::new_readonly(event_authority, false),
            AccountMeta::new_readonly(pump_program, false),
            AccountMeta::new_readonly(rent_sysvar, false),
        ],
        data,
    };

    run_migrate(
        &mut svm,
        &user,
        &ix,
        "SINGLE migrate call (ALL FOUR reserve fields = 0, associated_bonding_curve balance = 0, pool/lp_mint never created, no prior migration ever ran)",
    );

    println!("\nCONCLUSION: if this SUCCEEDED with zero inner instructions and the same");
    println!("\"Bonding curve already migrated\" log line, that confirms the check reads");
    println!("bonding_curve's own reserve fields (all four together), independent of");
    println!("whether pool/lp_mint exist. If it instead proceeded into real CPIs, none");
    println!("of bonding_curve's own fields are the check at all -- the real trigger");
    println!("must be pool (or some other account) already existing.");
}
