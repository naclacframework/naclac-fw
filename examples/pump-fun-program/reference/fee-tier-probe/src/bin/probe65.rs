//! Settles empirically what real `pump.so`'s `update_buyback_config` actually
//! does, beyond what 2 real mainnet transactions alone could show (signatures
//! `41P5xHGbEqQJgpZWthchDEvR6z6WZJPTyrV6XFwFHiQL4u31cByvQMcv9e1cVBcFC4ydT6PRnB82cv92mSJUi43u`
//! and `PgBhdehbVwsptSytw3JZ35aztEQ7hwErWHg7SqiigwBgxkRHSr1EtTjryK3axACJ82qivijYtTBAhmTw5uHY7DU`,
//! decoded via `getTransaction`): those two txs together showed the real
//! discriminator `[251,224,171,146,160,26,113,233]`, confirmed `Some(u64)` is
//! standard Borsh `Option<u64>` (1-byte tag + 8-byte LE value), and showed
//! one call with 0 extra accounts and one with 8 extra (read-only, non-signer
//! bare pubkeys) beyond the 4 declared in the real IDL (`global`, `authority`,
//! `event_authority`, `program`) -- strongly suggesting `remaining_accounts`
//! populate `Global.buyback_fee_recipients[8]`, optionally (0 or 8), mirroring
//! `set_params`'s own real `remaining_accounts[0]`/`remaining_accounts[1..8]`
//! -> `fee_recipient`/`fee_recipients[7]` convention (`probe21.rs`) -- but
//! `probe21` ALSO proved `set_params`'s own 8 accounts do NOT touch
//! `buyback_fee_recipients` at all, so this is a genuinely separate,
//! unconfirmed mechanism on this instruction specifically, not something to
//! infer by analogy. Never confirmed anywhere before this probe:
//! - Does `buyback_basis_points: None` leave `Global.buyback_basis_points`
//!   unchanged, or reset it to 0?
//! - Does 0 remaining_accounts really leave `buyback_fee_recipients`
//!   untouched (matching set_params's own account-count exactness), or does
//!   any other count (not just 0/8) work/fail differently?
//! - Is there a rent-exemption check on the 8 accounts, like `set_params` has
//!   on its own 8?
//!
//! Real `update_buyback_config` accounts (real IDL): global(mut),
//! authority(mut, signer, relations=[global]), event_authority, program.

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
const UPDATE_BUYBACK_CONFIG_DISCRIMINATOR: [u8; 8] = [251, 224, 171, 146, 160, 26, 113, 233];
const GLOBAL_DISCRIMINATOR: [u8; 8] = [167, 232, 232, 177, 200, 108, 114, 127];

// Real, full 24-field `Global` byte layout, offsets confirmed via
// `probe21.rs` (buyback_fee_recipients at 741..997) -- buyback_basis_points
// (u64) follows immediately after, at 997..1005.
const BUYBACK_FEE_RECIPIENTS_OFFSET: usize = 741;
const BUYBACK_BASIS_POINTS_OFFSET: usize = 741 + 256;

fn pk(s: &str) -> Pubkey {
    Pubkey::from_str(s).unwrap()
}

fn set_compute_unit_limit_ix(units: u32) -> Instruction {
    let mut data = vec![2u8];
    data.extend_from_slice(&units.to_le_bytes());
    Instruction { program_id: pk("ComputeBudget111111111111111111111111111111"), accounts: vec![], data }
}

fn global_account_data(authority: &Pubkey, buyback_fee_recipients: &Pubkey, buyback_basis_points: u64) -> Vec<u8> {
    let mut data = Vec::new();
    data.extend_from_slice(&GLOBAL_DISCRIMINATOR);
    data.push(1); // initialized
    data.extend_from_slice(authority.as_ref()); // authority
    data.extend_from_slice(&[0u8; 32]); // fee_recipient
    data.extend_from_slice(&[0u8; 8 * 5]); // initial_virtual_token_reserves..fee_basis_points
    data.extend_from_slice(&[0u8; 32]); // withdraw_authority
    data.push(0); // enable_migrate
    data.extend_from_slice(&[0u8; 8 * 2]); // pool_migration_fee, creator_fee_basis_points
    data.extend_from_slice(&[0u8; 32 * 7]); // fee_recipients[7]
    data.extend_from_slice(&[0u8; 32]); // set_creator_authority
    data.extend_from_slice(&[0u8; 32]); // admin_set_creator_authority
    data.push(0); // create_v2_enabled
    data.extend_from_slice(&[0u8; 32]); // whitelist_pda
    data.extend_from_slice(&[0u8; 32]); // reserved_fee_recipient
    data.push(0); // mayhem_mode_enabled
    data.extend_from_slice(&[0u8; 32 * 7]); // reserved_fee_recipients[7]
    data.push(0); // is_cashback_enabled
    for _ in 0..8 {
        data.extend_from_slice(buyback_fee_recipients.as_ref()); // buyback_fee_recipients[8]
    }
    data.extend_from_slice(&buyback_basis_points.to_le_bytes()); // buyback_basis_points
    data.extend_from_slice(&0u64.to_le_bytes()); // initial_virtual_quote_reserves
    // Real mainnet `Global.whitelisted_quote_mints` is `[Address; 1]` (confirmed,
    // `bonding-curve-05-batch1-v2-instructions.md`) -- our own project's
    // reimplementation later grew this to `[Address; 2]`, a deliberate
    // divergence, not the real layout this probe must match.
    data.extend_from_slice(&[0u8; 32]); // whitelisted_quote_mints[1]
    assert_eq!(data.len(), 1045, "Global layout drifted from probe21's confirmed offsets");
    data
}

fn update_buyback_config_ix_data(buyback_basis_points: Option<u64>) -> Vec<u8> {
    let mut data = UPDATE_BUYBACK_CONFIG_DISCRIMINATOR.to_vec();
    match buyback_basis_points {
        Some(v) => {
            data.push(1);
            data.extend_from_slice(&v.to_le_bytes());
        }
        None => data.push(0),
    }
    data
}

fn run_call(
    svm: &mut LiteSVM,
    label: &str,
    pump_program: Pubkey,
    global: Pubkey,
    event_authority: Pubkey,
    authority: &Keypair,
    buyback_basis_points: Option<u64>,
    remaining_accounts: &[Pubkey],
) {
    println!("=== {label} ===");
    println!("  buyback_basis_points arg = {buyback_basis_points:?}");
    println!("  remaining_accounts count = {}", remaining_accounts.len());

    let data = update_buyback_config_ix_data(buyback_basis_points);
    let mut accounts = vec![
        AccountMeta::new(global, false),
        AccountMeta::new(authority.pubkey(), true),
        AccountMeta::new_readonly(event_authority, false),
        AccountMeta::new_readonly(pump_program, false),
    ];
    for m in remaining_accounts {
        accounts.push(AccountMeta::new_readonly(*m, false));
    }

    let ix = Instruction { program_id: pump_program, accounts, data };
    let budget_ix = set_compute_unit_limit_ix(400_000);
    let blockhash = svm.latest_blockhash();
    let msg = Message::new(&[budget_ix, ix], Some(&authority.pubkey()));
    let tx = Transaction::new(&[authority], msg, blockhash);

    match svm.send_transaction(tx) {
        Ok(meta) => {
            println!("  RESULT: SUCCESS ({} CU)", meta.compute_units_consumed);
            for line in &meta.logs {
                if line.starts_with("Program log:") {
                    println!("  LOG: {line}");
                }
            }
        }
        Err(e) => {
            println!("  RESULT: FAILED: {:?}", e.err);
            for line in &e.meta.logs {
                if line.starts_with("Program log:") {
                    println!("  LOG: {line}");
                }
            }
        }
    }

    let global_after = svm.get_account(&global).unwrap();
    let stored_bps = u64::from_le_bytes(
        global_after.data[BUYBACK_BASIS_POINTS_OFFSET..BUYBACK_BASIS_POINTS_OFFSET + 8].try_into().unwrap(),
    );
    let stored_recipients: Vec<Pubkey> = (0..8)
        .map(|i| {
            Pubkey::try_from(
                &global_after.data[BUYBACK_FEE_RECIPIENTS_OFFSET + i * 32..BUYBACK_FEE_RECIPIENTS_OFFSET + i * 32 + 32],
            )
            .unwrap()
        })
        .collect();
    println!("  Global.buyback_basis_points after call = {stored_bps}");
    println!("  Global.buyback_fee_recipients after call = {stored_recipients:?}");
    println!();
}

fn main() {
    let pump_program = pk(PUMP_PROGRAM_ID);
    let mut svm = LiteSVM::new();
    let so_path = format!("{}/../pump-rust-client/artifacts/pump.so", env!("CARGO_MANIFEST_DIR"));
    svm.add_program_from_file(pump_program, &so_path).unwrap_or_else(|e| panic!("load pump.so: {e:?}"));
    svm.set_sysvar::<Clock>(&Clock { unix_timestamp: 1_700_000_000, slot: 100, epoch: 0, leader_schedule_epoch: 0, epoch_start_timestamp: 0 });

    let authority = Keypair::new();
    svm.airdrop(&authority.pubkey(), 10_000_000_000).unwrap();

    let initial_marker = Pubkey::new_unique();
    println!("initial_marker (buyback_fee_recipients[8] before any call) = {initial_marker}\n");

    let (global, _) = Pubkey::find_program_address(&[b"global"], &pump_program);
    svm.set_account(
        global,
        Account {
            lamports: 10_000_000,
            data: global_account_data(&authority.pubkey(), &initial_marker, 1234),
            owner: pump_program,
            executable: false,
            rent_epoch: 0,
        },
    )
    .unwrap();

    let (event_authority, _) = Pubkey::find_program_address(&[b"__event_authority"], &pump_program);

    // Call 1: Some(5000), 0 remaining_accounts -- expect bps=5000,
    // recipients still initial_marker (unchanged).
    run_call(&mut svm, "Call 1: Some(5000), 0 accounts", pump_program, global, event_authority, &authority, Some(5000), &[]);

    // Call 2: Some(7777), 8 fresh remaining_accounts -- expect bps=7777,
    // recipients replaced with these 8.
    let marker_new: Vec<Pubkey> = (0..8).map(|_| Pubkey::new_unique()).collect();
    println!("marker_new (8 accounts for call 2):");
    for (i, m) in marker_new.iter().enumerate() {
        println!("  [{i}] {m}");
    }
    println!();
    run_call(&mut svm, "Call 2: Some(7777), 8 accounts (marker_new)", pump_program, global, event_authority, &authority, Some(7777), &marker_new);

    // Call 3: None, 0 remaining_accounts -- THE key open question: does bps
    // stay 7777 (unchanged) or reset to 0? Recipients should stay marker_new.
    run_call(&mut svm, "Call 3: None, 0 accounts", pump_program, global, event_authority, &authority, None, &[]);

    // Call 4: None, 8 fresh remaining_accounts -- confirms recipients-update
    // is independent of the bps Option (works even when bps arg is None).
    let marker_new2: Vec<Pubkey> = (0..8).map(|_| Pubkey::new_unique()).collect();
    println!("marker_new2 (8 accounts for call 4):");
    for (i, m) in marker_new2.iter().enumerate() {
        println!("  [{i}] {m}");
    }
    println!();
    run_call(&mut svm, "Call 4: None, 8 accounts (marker_new2)", pump_program, global, event_authority, &authority, None, &marker_new2);

    // Call 5: Some(1), 3 remaining_accounts -- invalid count, expect a
    // rejection if the real program truly requires exactly 0 or 8.
    let marker_bad: Vec<Pubkey> = (0..3).map(|_| Pubkey::new_unique()).collect();
    run_call(&mut svm, "Call 5: Some(1), 3 accounts (expect failure)", pump_program, global, event_authority, &authority, Some(1), &marker_bad);
}
