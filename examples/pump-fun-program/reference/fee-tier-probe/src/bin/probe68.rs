//! Settles empirically what real `pump.so`'s `set_reserved_fee_recipients`
//! actually does. The real IDL's only declared arg is `whitelist_pda: pubkey`
//! and the only declared accounts are `global(mut)`, `authority(signer,
//! relations=[global])`, `event_authority`, `program` -- no account or arg
//! that could directly supply `Global.reserved_fee_recipient`/
//! `reserved_fee_recipients[7]`. But the real `ReservedFeeRecipientsEvent`
//! type (from the real IDL's `types`) has fields `reserved_fee_recipient:
//! pubkey` and `reserved_fee_recipients: [pubkey; 7]` -- NOT `whitelist_pda`
//! -- so the instruction changes those two fields by some mechanism the
//! declared args/accounts don't show. `probe21.rs` already proved the
//! precedent for this shape on `set_params` (undocumented 8 remaining_accounts
//! -> `fee_recipient`/`fee_recipients[7]`) and `probe65.rs` on
//! `update_buyback_config` (undocumented 0-or-8 remaining_accounts ->
//! `buyback_fee_recipients[8]`) -- both real IDLs were equally silent about
//! it. This probe tests whether `set_reserved_fee_recipients` follows the
//! same undocumented-remaining_accounts pattern (1 + 7 = 8, mirroring
//! `set_params`'s exact split), tests whether `whitelist_pda` is stored
//! literally into `Global.whitelist_pda`, and confirms whether the two
//! mechanisms are independent (as `update_buyback_config`'s bps-arg vs.
//! recipients-remaining_accounts turned out to be).
//!
//! Real `set_reserved_fee_recipients` discriminator:
//! [111,172,162,232,114,89,213,142].

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
const SET_RESERVED_FEE_RECIPIENTS_DISCRIMINATOR: [u8; 8] = [111, 172, 162, 232, 114, 89, 213, 142];
const GLOBAL_DISCRIMINATOR: [u8; 8] = [167, 232, 232, 177, 200, 108, 114, 127];

// Real, full 24-field `Global` byte layout, offsets confirmed via
// `probe21.rs`'s planted-marker method (disc=8, initialized=1, authority=32,
// fee_recipient=32, 5*u64=40, withdraw_authority=32, enable_migrate=1,
// pool_migration_fee=8, creator_fee_basis_points=8, fee_recipients[7]=224,
// set_creator_authority=32, admin_set_creator_authority=32,
// create_v2_enabled=1 -> whitelist_pda starts at 451).
const WHITELIST_PDA_OFFSET: usize = 451;
const RESERVED_FEE_RECIPIENT_OFFSET: usize = 483;
const RESERVED_FEE_RECIPIENTS_OFFSET: usize = 516;

fn pk(s: &str) -> Pubkey {
    Pubkey::from_str(s).unwrap()
}

fn set_compute_unit_limit_ix(units: u32) -> Instruction {
    let mut data = vec![2u8];
    data.extend_from_slice(&units.to_le_bytes());
    Instruction { program_id: pk("ComputeBudget111111111111111111111111111111"), accounts: vec![], data }
}

fn global_account_data(authority: &Pubkey) -> Vec<u8> {
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
    data.extend_from_slice(&[0u8; 32 * 8]); // buyback_fee_recipients[8]
    data.extend_from_slice(&0u64.to_le_bytes()); // buyback_basis_points
    data.extend_from_slice(&0u64.to_le_bytes()); // initial_virtual_quote_reserves
    data.extend_from_slice(&[0u8; 32]); // whitelisted_quote_mints[1]
    assert_eq!(data.len(), 1045, "Global layout drifted from probe21's confirmed offsets");
    data
}

fn ix_data(whitelist_pda: &Pubkey) -> Vec<u8> {
    let mut data = SET_RESERVED_FEE_RECIPIENTS_DISCRIMINATOR.to_vec();
    data.extend_from_slice(whitelist_pda.as_ref());
    data
}

fn run_call(
    svm: &mut LiteSVM,
    label: &str,
    pump_program: Pubkey,
    global: Pubkey,
    event_authority: Pubkey,
    authority: &Keypair,
    whitelist_pda: &Pubkey,
    remaining_accounts: &[Pubkey],
) {
    println!("=== {label} ===");
    println!("  whitelist_pda arg = {whitelist_pda}");
    println!("  remaining_accounts count = {}", remaining_accounts.len());

    let data = ix_data(whitelist_pda);
    let mut accounts = vec![
        AccountMeta::new(global, false),
        AccountMeta::new_readonly(authority.pubkey(), true),
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
            println!();
            return;
        }
    }

    let global_after = svm.get_account(&global).unwrap();
    let stored_whitelist_pda =
        Pubkey::try_from(&global_after.data[WHITELIST_PDA_OFFSET..WHITELIST_PDA_OFFSET + 32]).unwrap();
    let stored_reserved_fee_recipient =
        Pubkey::try_from(&global_after.data[RESERVED_FEE_RECIPIENT_OFFSET..RESERVED_FEE_RECIPIENT_OFFSET + 32])
            .unwrap();
    let stored_reserved_fee_recipients: Vec<Pubkey> = (0..7)
        .map(|i| {
            Pubkey::try_from(
                &global_after.data
                    [RESERVED_FEE_RECIPIENTS_OFFSET + i * 32..RESERVED_FEE_RECIPIENTS_OFFSET + i * 32 + 32],
            )
            .unwrap()
        })
        .collect();
    println!("  Global.whitelist_pda after call            = {stored_whitelist_pda}");
    println!("  Global.reserved_fee_recipient after call    = {stored_reserved_fee_recipient}");
    println!("  Global.reserved_fee_recipients[7] after call = {stored_reserved_fee_recipients:?}");
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

    let (global, _) = Pubkey::find_program_address(&[b"global"], &pump_program);
    svm.set_account(
        global,
        Account {
            lamports: 10_000_000,
            data: global_account_data(&authority.pubkey()),
            owner: pump_program,
            executable: false,
            rent_epoch: 0,
        },
    )
    .unwrap();

    let (event_authority, _) = Pubkey::find_program_address(&[b"__event_authority"], &pump_program);

    // Call 1: whitelist_pda arg only, 0 remaining_accounts -- does
    // whitelist_pda get stored literally? Do reserved_fee_recipient(s) move
    // at all with zero extra accounts?
    let whitelist_1 = Pubkey::new_unique();
    println!("whitelist_1 = {whitelist_1}\n");
    run_call(&mut svm, "Call 1: whitelist_pda arg, 0 accounts", pump_program, global, event_authority, &authority, &whitelist_1, &[]);

    // Call 2: whitelist_pda arg + 8 remaining_accounts, each pre-funded as a
    // rent-exempt System-owned wallet (call 1's real error --
    // `ConstraintRentExempt` on `reserved_fee_recipients[0]` -- showed each
    // of the 8 gets an individual rent-exemption check, mirroring
    // `set_params`'s own `fee_recipient`/`fee_recipients[7]` accounts in
    // `probe21.rs`). Tests whether remaining_accounts[0] ->
    // `reserved_fee_recipient` and remaining_accounts[1..8] ->
    // `reserved_fee_recipients[7]`, the same 1+7 split `probe21.rs` confirmed
    // for `set_params`.
    let whitelist_2 = Pubkey::new_unique();
    let marker_8: Vec<Pubkey> = (0..8).map(|_| Pubkey::new_unique()).collect();
    println!("whitelist_2 = {whitelist_2}");
    println!("marker_8 (8 accounts for call 2):");
    for (i, m) in marker_8.iter().enumerate() {
        println!("  [{i}] {m}");
        svm.set_account(*m, Account { lamports: 1_000_000, data: vec![], owner: pk("11111111111111111111111111111111"), executable: false, rent_epoch: 0 }).unwrap();
    }
    println!();
    run_call(&mut svm, "Call 2: whitelist_pda arg, 8 funded accounts (marker_8)", pump_program, global, event_authority, &authority, &whitelist_2, &marker_8);
}
