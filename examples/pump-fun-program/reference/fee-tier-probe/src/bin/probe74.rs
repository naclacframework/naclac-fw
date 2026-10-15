//! Settles empirically whether real `pump.so`'s `distribute_creator_fees`
//! (v1) rejects an uninitialized shareholder recipient
//! (`UnableToDistributeCreatorFeesToUninitializedAccount`) -- found by
//! reading the full real error list (not documented in the real IDL, and not
//! covered by `probe73.rs`, which only tested an *executable* recipient).
//! "Uninitialized" here means a plain System-owned account with zero
//! lamports and no data -- never `set_account`'d at all, i.e. genuinely
//! absent from the ledger, distinct from a funded-but-empty wallet.
//!
//! Same account shape as `probe73.rs`: `mint`, `bonding_curve`,
//! `sharing_config` (owned by `pump_fees`), `creator_vault` (mut),
//! `system_program`, `event_authority`, `program`, then one
//! `remaining_accounts` entry per shareholder.

use litesvm::LiteSVM;
use solana_sdk::{
    account::Account,
    clock::Clock,
    instruction::{AccountMeta, Instruction},
    message::Message,
    pubkey::Address,
    signature::{Keypair, Signer},
    transaction::Transaction,
};
use std::str::FromStr;

const PUMP_PROGRAM_ID: &str = "6EF8rrecthR5Dkzon8Nwu78hRvfCKubJ14M5uBEwF6P";
const PUMP_FEES_PROGRAM_ID: &str = "pfeeUxB6jkeY1Hxd7CsFCAjcbHA9rWtchMGdZ6VojVZ";
const TOKEN_PROGRAM_ID: &str = "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA";

const DISTRIBUTE_CREATOR_FEES_DISCRIMINATOR: [u8; 8] = [165, 114, 103, 0, 121, 206, 247, 81];
const BONDING_CURVE_DISCRIMINATOR: [u8; 8] = [23, 183, 248, 55, 96, 216, 172, 96];
const SHARING_CONFIG_DISCRIMINATOR: [u8; 8] = [216, 74, 9, 0, 56, 140, 93, 75];

fn pk(s: &str) -> Address {
    Address::from_str(s).unwrap()
}

fn set_compute_unit_limit_ix(units: u32) -> Instruction {
    let mut data = vec![2u8];
    data.extend_from_slice(&units.to_le_bytes());
    Instruction { program_id: pk("ComputeBudget111111111111111111111111111111"), accounts: vec![], data }
}

fn mint_account_data() -> Vec<u8> {
    let mut data = Vec::new();
    data.extend_from_slice(&0u32.to_le_bytes());
    data.extend_from_slice(&[0u8; 32]);
    data.extend_from_slice(&0u64.to_le_bytes());
    data.push(6);
    data.push(1);
    data.extend_from_slice(&0u32.to_le_bytes());
    data.extend_from_slice(&[0u8; 32]);
    data
}

fn bonding_curve_account_data(creator: &Address) -> Vec<u8> {
    let mut data = Vec::new();
    data.extend_from_slice(&BONDING_CURVE_DISCRIMINATOR);
    data.extend_from_slice(&[0u8; 8 * 5]);
    data.push(0); // complete
    data.extend_from_slice(creator.as_ref());
    data.push(0); // is_mayhem_mode
    data.push(0); // is_cashback_coin
    data.extend_from_slice(&Address::default().to_bytes()); // quote_mint
    data
}

fn sharing_config_account_data(bump: u8, mint: &Address, shareholder: &Address) -> Vec<u8> {
    let mut data = Vec::new();
    data.extend_from_slice(&SHARING_CONFIG_DISCRIMINATOR);
    data.push(bump);
    data.push(2); // version
    data.push(1); // status = Active
    data.extend_from_slice(mint.as_ref());
    data.extend_from_slice(shareholder.as_ref()); // admin
    data.push(0); // admin_revoked
    data.extend_from_slice(&1u32.to_le_bytes()); // shareholders_len = 1
    data.extend_from_slice(shareholder.as_ref()); // shareholders[0].address
    data.extend_from_slice(&10_000u16.to_le_bytes()); // share_bps
    data.resize(1024, 0);
    data
}

fn main() {
    let pump_program = pk(PUMP_PROGRAM_ID);
    let pump_fees_program = pk(PUMP_FEES_PROGRAM_ID);
    let mut svm = LiteSVM::new();
    let so_path = format!("{}/../pump-rust-client/artifacts/pump.so", env!("CARGO_MANIFEST_DIR"));
    svm.add_program_from_file(pump_program, &so_path).unwrap_or_else(|e| panic!("load pump.so: {e:?}"));
    svm.set_sysvar::<Clock>(&Clock { unix_timestamp: 1_700_000_000, slot: 100, epoch: 0, leader_schedule_epoch: 0, epoch_start_timestamp: 0 });

    let payer = Keypair::new();
    svm.airdrop(&payer.pubkey(), 10_000_000_000).unwrap();

    let mint = Address::new_unique();
    let (bonding_curve, _) = Address::find_program_address(&[b"bonding-curve", mint.as_ref()], &pump_program);
    let (sharing_config, sharing_config_bump) = Address::find_program_address(&[b"sharing-config", mint.as_ref()], &pump_fees_program);
    let creator = sharing_config;
    let (creator_vault, _) = Address::find_program_address(&[b"creator-vault", creator.as_ref()], &pump_program);
    let event_authority = Address::find_program_address(&[b"__event_authority"], &pump_program).0;

    // Uninitialized shareholder: a fresh pubkey never `set_account`'d --
    // genuinely absent from the ledger (System-owned, zero lamports, no
    // data by litesvm's default for any address never touched).
    let uninitialized_shareholder = Address::new_unique();

    svm.set_account(mint, Account { lamports: 10_000_000, data: mint_account_data(), owner: pk(TOKEN_PROGRAM_ID), executable: false, rent_epoch: 0 }).unwrap();
    svm.set_account(bonding_curve, Account { lamports: 10_000_000, data: bonding_curve_account_data(&creator), owner: pump_program, executable: false, rent_epoch: 0 }).unwrap();
    svm.set_account(
        sharing_config,
        Account { lamports: 10_000_000, data: sharing_config_account_data(sharing_config_bump, &mint, &uninitialized_shareholder), owner: pump_fees_program, executable: false, rent_epoch: 0 },
    )
    .unwrap();
    svm.set_account(creator_vault, Account { lamports: 5_000_000, data: vec![], owner: pk("11111111111111111111111111111111"), executable: false, rent_epoch: 0 }).unwrap();

    let accounts = vec![
        AccountMeta::new_readonly(mint, false),
        AccountMeta::new_readonly(bonding_curve, false),
        AccountMeta::new_readonly(sharing_config, false),
        AccountMeta::new(creator_vault, false),
        AccountMeta::new_readonly(pk("11111111111111111111111111111111"), false),
        AccountMeta::new_readonly(event_authority, false),
        AccountMeta::new_readonly(pump_program, false),
        AccountMeta::new(uninitialized_shareholder, false),
    ];
    let ix = Instruction { program_id: pump_program, accounts, data: DISTRIBUTE_CREATOR_FEES_DISCRIMINATOR.to_vec() };
    let budget_ix = set_compute_unit_limit_ix(400_000);
    let blockhash = svm.latest_blockhash();
    let msg = Message::new(&[budget_ix, ix], Some(&payer.pubkey()));
    let tx = Transaction::new(&[&payer], msg, blockhash);

    println!("=== distribute_creator_fees with an uninitialized shareholder ===");
    match svm.send_transaction(tx) {
        Ok(meta) => println!("  RESULT: SUCCESS ({} CU) -- no uninitialized-recipient rejection!", meta.compute_units_consumed),
        Err(e) => {
            println!("  RESULT: FAILED: {:?}", e.err);
            for line in &e.meta.logs {
                if line.starts_with("Program log:") {
                    println!("  LOG: {line}");
                }
            }
        }
    }

    // Second hypothesis: maybe it's `creator_vault` itself that must be
    // "uninitialized" to trigger this -- a bonding curve's very first ever
    // distribute, before any trade has funded it at all (genuinely absent
    // from the ledger, not just System-owned-with-zero-data as in every
    // other probe here).
    let mint2 = Address::new_unique();
    let (bonding_curve2, _) = Address::find_program_address(&[b"bonding-curve", mint2.as_ref()], &pump_program);
    let (sharing_config2, sharing_config2_bump) = Address::find_program_address(&[b"sharing-config", mint2.as_ref()], &pump_fees_program);
    let creator2 = sharing_config2;
    let (creator_vault2, _) = Address::find_program_address(&[b"creator-vault", creator2.as_ref()], &pump_program);
    let real_shareholder2 = Keypair::new();
    svm.airdrop(&real_shareholder2.pubkey(), 1_000_000_000).unwrap();

    svm.set_account(mint2, Account { lamports: 10_000_000, data: mint_account_data(), owner: pk(TOKEN_PROGRAM_ID), executable: false, rent_epoch: 0 }).unwrap();
    svm.set_account(bonding_curve2, Account { lamports: 10_000_000, data: bonding_curve_account_data(&creator2), owner: pump_program, executable: false, rent_epoch: 0 }).unwrap();
    svm.set_account(
        sharing_config2,
        Account { lamports: 10_000_000, data: sharing_config_account_data(sharing_config2_bump, &mint2, &real_shareholder2.pubkey()), owner: pump_fees_program, executable: false, rent_epoch: 0 },
    )
    .unwrap();
    // Deliberately NOT calling set_account on creator_vault2 -- it's
    // genuinely absent from the ledger, not just zero-lamport-with-data.

    let accounts2 = vec![
        AccountMeta::new_readonly(mint2, false),
        AccountMeta::new_readonly(bonding_curve2, false),
        AccountMeta::new_readonly(sharing_config2, false),
        AccountMeta::new(creator_vault2, false),
        AccountMeta::new_readonly(pk("11111111111111111111111111111111"), false),
        AccountMeta::new_readonly(event_authority, false),
        AccountMeta::new_readonly(pump_program, false),
        AccountMeta::new(real_shareholder2.pubkey(), false),
    ];
    let ix2 = Instruction { program_id: pump_program, accounts: accounts2, data: DISTRIBUTE_CREATOR_FEES_DISCRIMINATOR.to_vec() };
    let budget_ix2 = set_compute_unit_limit_ix(400_001);
    let blockhash2 = svm.latest_blockhash();
    let msg2 = Message::new(&[budget_ix2, ix2], Some(&payer.pubkey()));
    let tx2 = Transaction::new(&[&payer], msg2, blockhash2);

    println!("=== distribute_creator_fees with an uninitialized creator_vault (never funded) ===");
    match svm.send_transaction(tx2) {
        Ok(meta) => println!("  RESULT: SUCCESS ({} CU) -- no uninitialized-creator_vault rejection!", meta.compute_units_consumed),
        Err(e) => {
            println!("  RESULT: FAILED: {:?}", e.err);
            for line in &e.meta.logs {
                if line.starts_with("Program log:") {
                    println!("  LOG: {line}");
                }
            }
        }
    }
}
