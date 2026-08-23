//! Batch-tests the last 6 untested candidates from the definitive
//! writable-global instruction list (`node`-parsed from `pump.json`, not
//! grep): remove_quote_mint, set_virtual_quote_reserves,
//! toggle_cashback_enabled, toggle_create_v2, toggle_mayhem_mode,
//! update_global_authority. Combined with add_quote_mint/initialize/
//! set_params/update_buyback_config/set_reserved_fee_recipients (already
//! ruled out), this covers all 10 real instructions that ever write to
//! `Global` — if none of these touch `buyback_fee_recipients` either, no
//! real instruction sets it at all via any currently-documented path.

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
const GLOBAL_DISCRIMINATOR: [u8; 8] = [167, 232, 232, 177, 200, 108, 114, 127];

fn pk(s: &str) -> Pubkey {
    Pubkey::from_str(s).unwrap()
}

fn set_compute_unit_limit_ix(units: u32) -> Instruction {
    let mut data = vec![2u8];
    data.extend_from_slice(&units.to_le_bytes());
    Instruction { program_id: pk("ComputeBudget111111111111111111111111111111"), accounts: vec![], data }
}

fn global_account_data(authority: &Pubkey, buyback_marker: &Pubkey, whitelisted_quote_mint: &Pubkey) -> Vec<u8> {
    let mut data = Vec::new();
    data.extend_from_slice(&GLOBAL_DISCRIMINATOR);
    data.push(1);
    data.extend_from_slice(authority.as_ref());
    data.extend_from_slice(&[0u8; 32]);
    data.extend_from_slice(&[0u8; 8 * 5]);
    data.extend_from_slice(&[0u8; 32]);
    data.push(0);
    data.extend_from_slice(&[0u8; 8 * 2]);
    data.extend_from_slice(&[0u8; 32 * 7]);
    data.extend_from_slice(&[0u8; 32]);
    data.extend_from_slice(&[0u8; 32]);
    data.push(0);
    data.extend_from_slice(&[0u8; 32]);
    data.extend_from_slice(&[0u8; 32]);
    data.push(0);
    data.extend_from_slice(&[0u8; 32 * 7]);
    data.push(0);
    for _ in 0..8 {
        data.extend_from_slice(buyback_marker.as_ref());
    }
    data.extend_from_slice(&0u64.to_le_bytes());
    data.extend_from_slice(&0u64.to_le_bytes());
    data.extend_from_slice(whitelisted_quote_mint.as_ref()); // whitelisted_quote_mints[1]
    data
}

fn run_case(svm: &mut LiteSVM, pump_program: &Pubkey, label: &str, disc: [u8; 8], extra_args: &[u8], extra_accounts: Vec<AccountMeta>) {
    run_case_with_whitelist(svm, pump_program, label, disc, extra_args, extra_accounts, &Pubkey::default())
}

fn run_case_with_whitelist(
    svm: &mut LiteSVM,
    pump_program: &Pubkey,
    label: &str,
    disc: [u8; 8],
    extra_args: &[u8],
    extra_accounts: Vec<AccountMeta>,
    whitelisted_quote_mint: &Pubkey,
) {
    let authority = Keypair::new();
    svm.airdrop(&authority.pubkey(), 10_000_000_000).unwrap();
    let buyback_marker = Pubkey::new_unique();

    let (global, _) = Pubkey::find_program_address(&[b"global"], pump_program);
    svm.set_account(
        global,
        Account { lamports: 10_000_000, data: global_account_data(&authority.pubkey(), &buyback_marker, whitelisted_quote_mint), owner: *pump_program, executable: false, rent_epoch: 0 },
    )
    .unwrap();

    let (event_authority, _) = Pubkey::find_program_address(&[b"__event_authority"], pump_program);

    let mut data = disc.to_vec();
    data.extend_from_slice(extra_args);

    let mut accounts = vec![AccountMeta::new(global, false), AccountMeta::new(authority.pubkey(), true)];
    accounts.extend(extra_accounts);
    accounts.push(AccountMeta::new_readonly(event_authority, false));
    accounts.push(AccountMeta::new_readonly(*pump_program, false));

    let ix = Instruction { program_id: *pump_program, accounts, data };
    let budget_ix = set_compute_unit_limit_ix(400_000);
    let blockhash = svm.latest_blockhash();
    let msg = Message::new(&[budget_ix, ix], Some(&authority.pubkey()));
    let tx = Transaction::new(&[&authority], msg, blockhash);

    println!("\n=== {label} ===");
    match svm.send_transaction(tx) {
        Ok(_) => {
            let account = svm.get_account(&global).unwrap();
            let stored = &account.data[741..741 + 256];
            let unchanged = (0..8).all(|i| &stored[i * 32..i * 32 + 32] == buyback_marker.as_ref());
            println!("RESULT: SUCCESS. buyback_fee_recipients unchanged? {unchanged}");
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
    let mut svm = LiteSVM::new();
    let so_path = format!("{}/../pump-rust-client/artifacts/pump.so", env!("CARGO_MANIFEST_DIR"));
    svm.add_program_from_file(pump_program, &so_path).unwrap_or_else(|e| panic!("load pump.so: {e:?}"));
    svm.set_sysvar::<Clock>(&Clock { unix_timestamp: 1_700_000_000, slot: 100, epoch: 0, leader_schedule_epoch: 0, epoch_start_timestamp: 0 });

    let quote_mint = Pubkey::new_unique();
    // Previous run failed early on "quote mint not whitelisted" before ever
    // reaching whatever else the instruction does -- pre-whitelist it this
    // time so we actually see past that check.
    run_case_with_whitelist(
        &mut svm,
        &pump_program,
        "remove_quote_mint",
        [177, 65, 223, 38, 88, 209, 158, 155],
        quote_mint.as_ref(),
        vec![],
        &quote_mint,
    );
    run_case(&mut svm, &pump_program, "set_virtual_quote_reserves", [101, 135, 191, 104, 9, 88, 20, 96], &1_000_000u64.to_le_bytes(), vec![]);
    run_case(&mut svm, &pump_program, "toggle_cashback_enabled", [115, 103, 224, 255, 189, 89, 86, 195], &[1], vec![]);
    run_case(&mut svm, &pump_program, "toggle_create_v2", [28, 255, 230, 240, 172, 107, 203, 171], &[1], vec![]);
    run_case(&mut svm, &pump_program, "toggle_mayhem_mode", [1, 9, 111, 208, 100, 31, 255, 163], &[1], vec![]);
    let new_authority = Pubkey::new_unique();
    run_case(
        &mut svm,
        &pump_program,
        "update_global_authority",
        [227, 181, 74, 196, 208, 21, 97, 213],
        &[],
        vec![AccountMeta::new_readonly(new_authority, false)],
    );
}
