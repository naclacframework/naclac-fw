//! Settles empirically (not by array-size inference) what real `pump.so`'s
//! `SetParamsEvent.fee_recipients: [pubkey; 8]` field actually mirrors.
//! `Global` has two 7-sized pubkey arrays (`fee_recipients`,
//! `reserved_fee_recipients`) and one 8-sized array
//! (`buyback_fee_recipients`) — this probe puts a DISTINCT, recognizable
//! pubkey in each of the three arrays, calls real `set_params`, and reads
//! back exactly which one appears in the emitted event's `fee_recipients`
//! slot.
//!
//! Real `set_params`: discriminator [27,234,178,52,147,2,187,141]. Accounts:
//! global(mut), authority(signer, must equal global.authority),
//! event_authority, program. No `bonding_curve` account at all.

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
const SET_PARAMS_DISCRIMINATOR: [u8; 8] = [27, 234, 178, 52, 147, 2, 187, 141];
const GLOBAL_DISCRIMINATOR: [u8; 8] = [167, 232, 232, 177, 200, 108, 114, 127];
const SET_PARAMS_EVENT_DISCRIMINATOR_SEARCH_LEN: usize = 8;

fn pk(s: &str) -> Pubkey {
    Pubkey::from_str(s).unwrap()
}

fn set_compute_unit_limit_ix(units: u32) -> Instruction {
    let mut data = vec![2u8];
    data.extend_from_slice(&units.to_le_bytes());
    Instruction { program_id: pk("ComputeBudget111111111111111111111111111111"), accounts: vec![], data }
}

// Full real 24-field Global, with three DISTINCT recognizable pubkeys
// planted in fee_recipients[7] (marker_a), reserved_fee_recipients[7]
// (marker_b), and buyback_fee_recipients[8] (marker_c).
fn global_account_data(authority: &Pubkey, marker_a: &Pubkey, marker_b: &Pubkey, marker_c: &Pubkey) -> Vec<u8> {
    let mut data = Vec::new();
    data.extend_from_slice(&GLOBAL_DISCRIMINATOR);
    data.push(1); // initialized
    data.extend_from_slice(authority.as_ref()); // authority
    data.extend_from_slice(&[0u8; 32]); // fee_recipient
    data.extend_from_slice(&[0u8; 8 * 5]); // initial_virtual_token_reserves..fee_basis_points
    data.extend_from_slice(&[0u8; 32]); // withdraw_authority
    data.push(0); // enable_migrate
    data.extend_from_slice(&[0u8; 8 * 2]); // pool_migration_fee, creator_fee_basis_points
    for _ in 0..7 {
        data.extend_from_slice(marker_a.as_ref()); // fee_recipients[7]
    }
    data.extend_from_slice(&[0u8; 32]); // set_creator_authority
    data.extend_from_slice(&[0u8; 32]); // admin_set_creator_authority
    data.push(0); // create_v2_enabled
    data.extend_from_slice(&[0u8; 32]); // whitelist_pda
    data.extend_from_slice(&[0u8; 32]); // reserved_fee_recipient
    data.push(0); // mayhem_mode_enabled
    for _ in 0..7 {
        data.extend_from_slice(marker_b.as_ref()); // reserved_fee_recipients[7]
    }
    data.push(0); // is_cashback_enabled
    for _ in 0..8 {
        data.extend_from_slice(marker_c.as_ref()); // buyback_fee_recipients[8]
    }
    data.extend_from_slice(&0u64.to_le_bytes()); // buyback_basis_points
    data.extend_from_slice(&0u64.to_le_bytes()); // initial_virtual_quote_reserves
    data.extend_from_slice(&[0u8; 32]); // whitelisted_quote_mints[1]
    data
}

fn main() {
    let pump_program = pk(PUMP_PROGRAM_ID);
    let mut svm = LiteSVM::new();
    let so_path = format!("{}/../pump-rust-client/artifacts/pump.so", env!("CARGO_MANIFEST_DIR"));
    svm.add_program_from_file(pump_program, &so_path).unwrap_or_else(|e| panic!("load pump.so: {e:?}"));
    svm.set_sysvar::<Clock>(&Clock { unix_timestamp: 1_700_000_000, slot: 100, epoch: 0, leader_schedule_epoch: 0, epoch_start_timestamp: 0 });

    let authority = Keypair::new();
    svm.airdrop(&authority.pubkey(), 10_000_000_000).unwrap();

    let marker_a = Pubkey::new_unique(); // fee_recipients[7]
    let marker_b = Pubkey::new_unique(); // reserved_fee_recipients[7]
    let marker_c = Pubkey::new_unique(); // buyback_fee_recipients[8]
    println!("marker_a (fee_recipients[7])          = {marker_a}");
    println!("marker_b (reserved_fee_recipients[7])  = {marker_b}");
    println!("marker_c (buyback_fee_recipients[8])   = {marker_c}\n");

    let (global, _) = Pubkey::find_program_address(&[b"global"], &pump_program);
    svm.set_account(
        global,
        Account {
            lamports: 10_000_000,
            data: global_account_data(&authority.pubkey(), &marker_a, &marker_b, &marker_c),
            owner: pump_program,
            executable: false,
            rent_epoch: 0,
        },
    )
    .unwrap();

    let (event_authority, _) = Pubkey::find_program_address(&[b"__event_authority"], &pump_program);

    let mut data = SET_PARAMS_DISCRIMINATOR.to_vec();
    data.extend_from_slice(&1_073_000_000_000_000u64.to_le_bytes()); // initial_virtual_token_reserves
    data.extend_from_slice(&30_000_000_000u64.to_le_bytes()); // initial_virtual_sol_reserves
    data.extend_from_slice(&793_100_000_000_000u64.to_le_bytes()); // initial_real_token_reserves
    data.extend_from_slice(&1_000_000_000_000_000u64.to_le_bytes()); // token_total_supply
    data.extend_from_slice(&0u64.to_le_bytes()); // fee_basis_points
    data.extend_from_slice(authority.pubkey().as_ref()); // withdraw_authority
    data.push(0); // enable_migrate
    data.extend_from_slice(&0u64.to_le_bytes()); // pool_migration_fee
    data.extend_from_slice(&0u64.to_le_bytes()); // creator_fee_basis_points
    data.extend_from_slice(&[0u8; 32]); // set_creator_authority
    data.extend_from_slice(&[0u8; 32]); // admin_set_creator_authority

    // Real `set_params` demands exactly 8 remaining accounts
    // (`NotEnoughRemainingAccounts`, Left=0 Right=8) — not documented in the
    // real IDL's `args` list. Supplying 8 fresh, distinct marker pubkeys
    // (marker_d) to see if they land in the emitted event and/or Global.
    // Testing 16 now (8 more than confirmed needed) to see whether
    // buyback_fee_recipients[8] is ALSO settable via a second block of 8
    // remaining accounts that the stale IDL doesn't show either.
    let marker_d: Vec<Pubkey> = (0..16).map(|_| Pubkey::new_unique()).collect();
    println!("marker_d (16 remaining_accounts passed to set_params, testing beyond the confirmed 8):");
    for (i, m) in marker_d.iter().enumerate() {
        println!("  [{i}] {m}");
        // Real `fee_recipients[N]` needs `ConstraintRentExempt` — fund each
        // as a plain rent-exempt System-owned wallet.
        svm.set_account(*m, Account { lamports: 1_000_000, data: vec![], owner: pk("11111111111111111111111111111111"), executable: false, rent_epoch: 0 }).unwrap();
    }
    println!();

    let mut accounts = vec![
        AccountMeta::new(global, false),
        AccountMeta::new(authority.pubkey(), true),
        AccountMeta::new_readonly(event_authority, false),
        AccountMeta::new_readonly(pump_program, false),
    ];
    for m in &marker_d {
        accounts.push(AccountMeta::new_readonly(*m, false));
    }

    let ix = Instruction { program_id: pump_program, accounts, data };

    let budget_ix = set_compute_unit_limit_ix(400_000);
    let blockhash = svm.latest_blockhash();
    let msg = Message::new(&[budget_ix, ix], Some(&authority.pubkey()));
    let tx = Transaction::new(&[&authority], msg, blockhash);

    match svm.send_transaction(tx) {
        Ok(meta) => {
            println!("RESULT: SUCCESS");
            for line in &meta.logs {
                println!("  LOG: {line}");
                if let Some(b64) = line.strip_prefix("Program data: ") {
                    if let Ok(raw) = base64_decode(b64) {
                        println!("  (raw event bytes, {} bytes, disc={:02x?})", raw.len(), &raw[0..SET_PARAMS_EVENT_DISCRIMINATOR_SEARCH_LEN.min(raw.len())]);
                        // Scan the whole decoded event for occurrences of each marker pubkey.
                        for (label, marker) in [("marker_a/fee_recipients[7]", marker_a), ("marker_b/reserved_fee_recipients[7]", marker_b), ("marker_c/buyback_fee_recipients[8]", marker_c)] {
                            let needle = marker.to_bytes();
                            let found = raw.windows(32).any(|w| w == needle);
                            println!("    contains {label}? {found}");
                        }
                        let marker_d_found = marker_d.iter().all(|m| raw.windows(32).any(|w| w == m.to_bytes()));
                        println!("    contains ALL of marker_d (the 8 remaining_accounts)? {marker_d_found}");
                    }
                }
            }

            let global_after = svm.get_account(&global).unwrap();

            // Correct offsets (disc=8, initialized=1, authority=32, THEN
            // fee_recipient=32 at [41..73), then 5*u64(40)+withdraw_authority(32)+
            // enable_migrate(1)+pool_migration_fee(8)+creator_fee_basis_points(8)
            // = 89 more bytes -> fee_recipients[7] starts at 73+89=162).
            let stored_fee_recipient = &global_after.data[41..73];
            let stored_fee_recipients7 = &global_after.data[162..162 + 224];
            let fee_recipient_matches_marker_d0 = stored_fee_recipient == marker_d[0].as_ref();
            let fee_recipients7_matches_marker_d_rest =
                (0..7).all(|i| &stored_fee_recipients7[i * 32..i * 32 + 32] == marker_d[i + 1].as_ref());
            println!("\nGlobal.fee_recipient after the call matches marker_d[0]? {fee_recipient_matches_marker_d0}");
            println!("Global.fee_recipients[7] after the call matches marker_d[1..8]? {fee_recipients7_matches_marker_d_rest}");
            println!(
                "Global.fee_recipient raw bytes: {:?}",
                Pubkey::try_from(stored_fee_recipient).map(|p| p.to_string())
            );

            // Did EITHER block of 8 remaining accounts get WRITTEN into
            // Global's own buyback_fee_recipients[8] (offset 741, 256 bytes)?
            let stored = &global_after.data[741..741 + 256];
            let matches_marker_d_first8 = (0..8).all(|i| &stored[i * 32..i * 32 + 32] == marker_d[i].as_ref());
            let matches_marker_d_second8 = (0..8).all(|i| &stored[i * 32..i * 32 + 32] == marker_d[i + 8].as_ref());
            let matches_marker_c = (0..8).all(|i| &stored[i * 32..i * 32 + 32] == marker_c.as_ref());
            println!("\nGlobal.buyback_fee_recipients[8] after the call:");
            println!("  matches marker_d[0..8] (first block passed in)? {matches_marker_d_first8}");
            println!("  matches marker_d[8..16] (second block passed in)? {matches_marker_d_second8}");
            println!("  still matches marker_c (unchanged from before the call)? {matches_marker_c}");
        }
        Err(e) => {
            println!("RESULT: FAILED: {:?}", e.err);
            for line in &e.meta.logs {
                println!("  LOG: {line}");
            }
        }
    }
}

fn base64_decode(input: &str) -> Result<Vec<u8>, ()> {
    const TABLE: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut rev = [255u8; 256];
    for (i, &c) in TABLE.iter().enumerate() {
        rev[c as usize] = i as u8;
    }
    let clean: Vec<u8> = input.bytes().filter(|&b| b != b'=').collect();
    let mut out = Vec::with_capacity(clean.len() * 3 / 4);
    for chunk in clean.chunks(4) {
        let vals: Vec<u8> = chunk.iter().map(|&b| rev[b as usize]).collect();
        if vals.iter().any(|&v| v == 255) {
            return Err(());
        }
        out.push((vals[0] << 2) | (vals.get(1).copied().unwrap_or(0) >> 4));
        if vals.len() > 2 {
            out.push((vals[1] << 4) | (vals[2] >> 2));
        }
        if vals.len() > 3 {
            out.push((vals[2] << 6) | vals[3]);
        }
    }
    Ok(out)
}
