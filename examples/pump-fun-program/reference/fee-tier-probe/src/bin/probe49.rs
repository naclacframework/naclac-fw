//! Direct, blind verification of the confirmed real formula
//! `virtual_quote_reserves = floor(quote_amount_in * base_amount_in / 1e15)`
//! (derived from 5 independent real mainnet `migrate_v2` transactions, see
//! `docs/plan/amm-03-boost-mechanism.md`) -- runs the REAL, proven
//! `migrate_v2` -> `create_pool` -> `init_boost` CPI chain (same pattern as
//! `probe45.rs`/`probe46.rs`, not the standalone dummy-CPI method that gave
//! a different, still-unexplained result) against fresh, previously-untested
//! numbers, using WSOL as the quote mint (matching all 5 real examples,
//! 9 decimals) instead of USDC (6 decimals, used in every earlier probe) --
//! also directly tests whether quote-mint decimals affect the formula.
//! The predicted value is computed and printed BEFORE submitting the
//! transaction, so this is a real blind check, not a post-hoc fit.
//!
//! Usage: cargo run --bin probe49 [-- <rpc-url>]

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

const PUMP_PROGRAM_ID: &str = "6EF8rrecthR5Dkzon8Nwu78hRvfCKubJ14M5uBEwF6P";
const PUMP_AMM_PROGRAM_ID: &str = "pAMMBay6oceH9fJKBRHGP5D4bD4sWpmSwMn52FMfXEA";
const ASSOCIATED_TOKEN_PROGRAM_ID: &str = "ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL";
const SYSTEM_PROGRAM_ID: &str = "11111111111111111111111111111111";
const TOKEN_PROGRAM_ID: &str = "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA";
const TOKEN_2022_PROGRAM_ID: &str = "TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb";
const RENT_SYSVAR_ID: &str = "SysvarRent111111111111111111111111111111111";

const MIGRATE_V2_DISCRIMINATOR: [u8; 8] = [187, 203, 18, 31, 206, 237, 254, 41];
const BONDING_CURVE_DISCRIMINATOR: [u8; 8] = [23, 183, 248, 55, 96, 216, 172, 96];

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

fn bonding_curve_account_data(
    virtual_token_reserves: u64,
    virtual_quote_reserves: u64,
    real_token_reserves: u64,
    real_quote_reserves: u64,
    token_total_supply: u64,
    complete: bool,
    creator: &Pubkey,
    quote_mint: &Pubkey,
) -> Vec<u8> {
    let mut data = Vec::new();
    data.extend_from_slice(&BONDING_CURVE_DISCRIMINATOR);
    data.extend_from_slice(&virtual_token_reserves.to_le_bytes());
    data.extend_from_slice(&virtual_quote_reserves.to_le_bytes());
    data.extend_from_slice(&real_token_reserves.to_le_bytes());
    data.extend_from_slice(&real_quote_reserves.to_le_bytes());
    data.extend_from_slice(&token_total_supply.to_le_bytes());
    data.push(if complete { 1 } else { 0 });
    data.extend_from_slice(creator.as_ref());
    data.push(0);
    data.push(0);
    data.extend_from_slice(quote_mint.as_ref());
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

fn main() {
    let rpc_url = std::env::args().nth(1).unwrap_or_else(|| "https://api.mainnet-beta.solana.com".to_string());
    let client = RpcClient::new_with_timeout(rpc_url, Duration::from_secs(60));

    let pump_program = pk(PUMP_PROGRAM_ID);
    let pump_amm_program = pk(PUMP_AMM_PROGRAM_ID);
    let system_program = pk(SYSTEM_PROGRAM_ID);
    let base_token_program = pk(TOKEN_PROGRAM_ID);
    let quote_token_program = pk(TOKEN_PROGRAM_ID); // real examples: WSOL uses classic Token, not Token-2022
    let token_2022_program = pk(TOKEN_2022_PROGRAM_ID);
    let associated_token_program = pk(ASSOCIATED_TOKEN_PROGRAM_ID);
    let rent_sysvar = pk(RENT_SYSVAR_ID);
    // Deliberately NOT the real WSOL address -- a fresh synthetic mint with
    // WSOL's real decimal count (9) but none of WSOL's special native-mint
    // account-balance invariants (which a hand-fixtured litesvm account
    // can't satisfy without replicating SPL Token's native-mint handling
    // exactly, and isn't needed here -- what's actually being tested is
    // whether quote-mint DECIMALS affect the boost formula, not WSOL
    // specifically).
    let quote_mint = Pubkey::new_unique();

    let mut svm = LiteSVM::new();
    for (program_id, so_name) in [(pump_program, "pump.so"), (pump_amm_program, "pump_amm.so")] {
        let so_path = format!("{}/../pump-rust-client/artifacts/{}", env!("CARGO_MANIFEST_DIR"), so_name);
        svm.add_program_from_file(program_id, &so_path).unwrap_or_else(|e| panic!("load {so_name}: {e:?}"));
    }
    svm.set_sysvar::<Clock>(&Clock { unix_timestamp: 1_700_000_000, slot: 100, epoch: 0, leader_schedule_epoch: 0, epoch_start_timestamp: 0 });

    let (global, _) = Pubkey::find_program_address(&[b"global"], &pump_program);
    let global_account = fetch_account(&client, &global).expect("fetch global failed");
    let withdraw_authority = Pubkey::try_from(&global_account.data[113..145]).expect("withdraw_authority pubkey");
    svm.set_account(global, global_account.clone()).unwrap();

    let (amm_global_config, _) = Pubkey::find_program_address(&[b"global_config"], &pump_amm_program);
    let amm_global_config_account = fetch_account(&client, &amm_global_config).expect("fetch amm global_config failed");
    svm.set_account(amm_global_config, amm_global_config_account).unwrap();

    svm.set_account(quote_mint, Account { lamports: 10_000_000, data: mint_account_data(9), owner: quote_token_program, executable: false, rent_epoch: 0 }).unwrap();

    let creator = Pubkey::new_unique();
    let base_mint = Pubkey::new_unique();
    svm.set_account(base_mint, Account { lamports: 10_000_000, data: mint_account_data(6), owner: base_token_program, executable: false, rent_epoch: 0 }).unwrap();

    let (bonding_curve, _) = Pubkey::find_program_address(&[b"bonding-curve", base_mint.as_ref()], &pump_program);
    // Fresh, previously-untested numbers -- not reused from any earlier probe.
    let base_funding: u64 = 350_000_000_000_000;
    let real_quote_reserves: u64 = 9_876_543_210;
    let total_supply_constant: u128 = 1_000_000_000_000_000;
    let predicted_virtual_quote_reserves = (real_quote_reserves as u128 * base_funding as u128 / total_supply_constant) as u64;
    println!("PREDICTED virtual_quote_reserves (formula: quote*base/1e15) = {predicted_virtual_quote_reserves}");
    println!("(base_funding={base_funding}, real_quote_reserves={real_quote_reserves}, quote_mint=WSOL/9dp)");

    let bc_data = bonding_curve_account_data(0, 30_000_000_000, 0, real_quote_reserves, 1_000_000_000_000_000, true, &creator, &quote_mint);
    svm.set_account(bonding_curve, Account { lamports: 50_000_000, data: bc_data, owner: pump_program, executable: false, rent_epoch: 0 }).unwrap();

    let associated_base_bonding_curve = ata_address(&bonding_curve, &base_mint, &base_token_program);
    svm.set_account(
        associated_base_bonding_curve,
        Account { lamports: 2_039_280, data: token_account_data(&base_mint, &bonding_curve, base_funding), owner: base_token_program, executable: false, rent_epoch: 0 },
    ).unwrap();
    let associated_quote_bonding_curve = ata_address(&bonding_curve, &quote_mint, &quote_token_program);
    svm.set_account(
        associated_quote_bonding_curve,
        Account { lamports: 2_039_280, data: token_account_data(&quote_mint, &bonding_curve, real_quote_reserves), owner: quote_token_program, executable: false, rent_epoch: 0 },
    ).unwrap();

    let user = Keypair::new();
    svm.airdrop(&user.pubkey(), 100_000_000_000).unwrap();

    let (pool_authority, _) = Pubkey::find_program_address(&[b"pool-authority", base_mint.as_ref()], &pump_program);
    let (pool, _) = Pubkey::find_program_address(
        &[b"pool", &0u16.to_le_bytes(), pool_authority.as_ref(), base_mint.as_ref(), quote_mint.as_ref()],
        &pump_amm_program,
    );
    let pool_authority_mint_account = ata_address(&pool_authority, &base_mint, &base_token_program);
    let pool_authority_quote_account = ata_address(&pool_authority, &quote_mint, &quote_token_program);
    let (lp_mint, _) = Pubkey::find_program_address(&[b"pool_lp_mint", pool.as_ref()], &pump_amm_program);
    let user_pool_token_account = ata_address(&pool_authority, &lp_mint, &token_2022_program);
    let pool_base_token_account = ata_address(&pool, &base_mint, &base_token_program);
    let pool_quote_token_account = ata_address(&pool, &quote_mint, &quote_token_program);
    let (pump_amm_event_authority, _) = Pubkey::find_program_address(&[b"__event_authority"], &pump_amm_program);
    let (event_authority, _) = Pubkey::find_program_address(&[b"__event_authority"], &pump_program);

    let (boost_vault_authority, _) = Pubkey::find_program_address(&[b"boost_vault", pool.as_ref()], &pump_amm_program);
    let boost_vault = ata_address(&boost_vault_authority, &quote_mint, &quote_token_program);

    let ix = Instruction {
        program_id: pump_program,
        accounts: vec![
            AccountMeta::new_readonly(global, false),
            AccountMeta::new(withdraw_authority, false),
            AccountMeta::new_readonly(base_mint, false),
            AccountMeta::new_readonly(quote_mint, false),
            AccountMeta::new(bonding_curve, false),
            AccountMeta::new(associated_base_bonding_curve, false),
            AccountMeta::new(associated_quote_bonding_curve, false),
            AccountMeta::new(user.pubkey(), true),
            AccountMeta::new_readonly(system_program, false),
            AccountMeta::new_readonly(pump_amm_program, false),
            AccountMeta::new(pool, false),
            AccountMeta::new(pool_authority, false),
            AccountMeta::new(pool_authority_mint_account, false),
            AccountMeta::new(pool_authority_quote_account, false),
            AccountMeta::new_readonly(amm_global_config, false),
            AccountMeta::new(lp_mint, false),
            AccountMeta::new(user_pool_token_account, false),
            AccountMeta::new(pool_base_token_account, false),
            AccountMeta::new(pool_quote_token_account, false),
            AccountMeta::new_readonly(base_token_program, false),
            AccountMeta::new_readonly(quote_token_program, false),
            AccountMeta::new_readonly(token_2022_program, false),
            AccountMeta::new_readonly(associated_token_program, false),
            AccountMeta::new_readonly(pump_amm_event_authority, false),
            AccountMeta::new_readonly(rent_sysvar, false),
            AccountMeta::new_readonly(event_authority, false),
            AccountMeta::new_readonly(pump_program, false),
            AccountMeta::new(boost_vault_authority, false),
            AccountMeta::new(boost_vault, false),
        ],
        data: MIGRATE_V2_DISCRIMINATOR.to_vec(),
    };

    let budget_ix = set_compute_unit_limit_ix(400_000);
    let blockhash = svm.latest_blockhash();
    let msg = Message::new(&[budget_ix, ix], Some(&user.pubkey()));
    let account_keys: Vec<Pubkey> = msg.account_keys.clone();
    let tx = Transaction::new(&[&user], msg, blockhash);

    println!("\nSubmitting migrate_v2 (WSOL-paired, fresh numbers)...");
    match svm.send_transaction(tx) {
        Ok(meta) => {
            println!("RESULT: SUCCESS, {} CU", meta.compute_units_consumed);
            let actual = token_balance(&svm, &boost_vault);
            println!("ACTUAL boost_vault balance (= virtual_quote_reserves) = {actual:?}");
            println!("PREDICTED = {predicted_virtual_quote_reserves}");
            match actual {
                Some(a) if a == predicted_virtual_quote_reserves => println!("==> EXACT MATCH"),
                Some(a) => println!("==> MISMATCH, diff = {}", a as i128 - predicted_virtual_quote_reserves as i128),
                None => println!("==> boost_vault does not exist"),
            }
            if let Some(acc) = svm.get_account(&pool) {
                let vqr = i128::from_le_bytes(acc.data[245..261].try_into().unwrap());
                println!("pool.virtual_quote_reserves = {vqr}");
            }
            for top_level in &meta.inner_instructions {
                for inner in top_level {
                    let prog_idx = inner.instruction.program_id_index as usize;
                    let Some(prog) = account_keys.get(prog_idx) else { continue };
                    if *prog != pump_amm_program || inner.instruction.accounts.len() != 1 {
                        continue;
                    }
                    let data = &inner.instruction.data;
                    const INIT_BOOST_EVENT_DISC: [u8; 8] = [174, 124, 74, 249, 4, 81, 246, 17];
                    if data.len() >= 16 && data[8..16] == INIT_BOOST_EVENT_DISC {
                        let off = 16 + 8 + 32 + 32 + 32;
                        if data.len() >= off + 24 {
                            let vqr = i128::from_le_bytes(data[off..off + 16].try_into().unwrap());
                            let real_after = u64::from_le_bytes(data[off + 16..off + 24].try_into().unwrap());
                            println!("InitBoostEvent (from event): virtual_quote_reserves={vqr} real_quote_reserves_after={real_after}");
                        }
                    }
                }
            }
        }
        Err(e) => {
            println!("RESULT: FAILED: {:?}", e.err);
            for l in &e.meta.logs {
                println!("  LOG: {l}");
            }
        }
    }
}
