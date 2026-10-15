//! Settles empirically what real `pump.so`'s `close_user_volume_accumulator`
//! actually requires/does. The real event
//! (`CloseUserVolumeAccumulatorEvent{user, timestamp, total_unclaimed_tokens,
//! total_claimed_tokens, current_sol_volume, last_update_timestamp}`)
//! conspicuously omits `cashback_earned`/`stable_cashback_earned` -- this
//! probe checks whether closing an account with nonzero values in those
//! (and in `total_unclaimed_tokens`) is rejected outright, or silently
//! allowed (forfeiting them), by running the real bytecode directly.
//!
//! Real accounts (`pump-public-docs/idl/pump.json`): `user` (signer, mut),
//! `user_volume_accumulator` (mut, pda=[seed, user]), `event_authority`,
//! `program`. No `system_program` in the real account list -- Anchor's
//! `close = user` constraint moves lamports directly, no CPI needed.
//!
//! Real `UserVolumeAccumulator` is 137 bytes total (confirmed elsewhere in
//! this repo, `fees-07-donation-relay-progress.md`): 8 disc + 98 documented
//! fields + 31 reserved/padding bytes.

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
const CLOSE_USER_VOLUME_ACCUMULATOR_DISCRIMINATOR: [u8; 8] = [249, 69, 164, 218, 150, 103, 84, 138];
const USER_VOLUME_ACCUMULATOR_DISCRIMINATOR: [u8; 8] = [86, 255, 112, 14, 102, 53, 154, 250];

fn pk(s: &str) -> Address {
    Address::from_str(s).unwrap()
}

fn set_compute_unit_limit_ix(units: u32) -> Instruction {
    let mut data = vec![2u8];
    data.extend_from_slice(&units.to_le_bytes());
    Instruction { program_id: pk("ComputeBudget111111111111111111111111111111"), accounts: vec![], data }
}

/// Builds a real-shaped `UserVolumeAccumulator` (137 bytes total).
#[allow(clippy::too_many_arguments)]
fn user_volume_accumulator_data(
    user: &Address,
    needs_claim: bool,
    total_unclaimed_tokens: u64,
    total_claimed_tokens: u64,
    current_sol_volume: u64,
    last_update_timestamp: i64,
    has_total_claimed_tokens: bool,
    cashback_earned: u64,
    total_cashback_claimed: u64,
    stable_cashback_earned: u64,
    total_stable_cashback_claimed: u64,
) -> Vec<u8> {
    let mut data = Vec::new();
    data.extend_from_slice(&USER_VOLUME_ACCUMULATOR_DISCRIMINATOR);
    data.extend_from_slice(user.as_ref());
    data.push(needs_claim as u8);
    data.extend_from_slice(&total_unclaimed_tokens.to_le_bytes());
    data.extend_from_slice(&total_claimed_tokens.to_le_bytes());
    data.extend_from_slice(&current_sol_volume.to_le_bytes());
    data.extend_from_slice(&last_update_timestamp.to_le_bytes());
    data.push(has_total_claimed_tokens as u8);
    data.extend_from_slice(&cashback_earned.to_le_bytes());
    data.extend_from_slice(&total_cashback_claimed.to_le_bytes());
    data.extend_from_slice(&stable_cashback_earned.to_le_bytes());
    data.extend_from_slice(&total_stable_cashback_claimed.to_le_bytes());
    data.resize(137, 0);
    data
}

fn run_close(
    svm: &mut LiteSVM,
    label: &str,
    call_index: u32,
    pump_program: Address,
    build_data: impl FnOnce(&Address) -> Vec<u8>,
    uva_lamports: u64,
) {
    let user = Keypair::new();
    svm.airdrop(&user.pubkey(), 10_000_000_000).unwrap();

    let (uva_pda, _) = Address::find_program_address(&[b"user_volume_accumulator", user.pubkey().as_ref()], &pump_program);
    let data = build_data(&user.pubkey());
    svm.set_account(uva_pda, Account { lamports: uva_lamports, data, owner: pump_program, executable: false, rent_epoch: 0 }).unwrap();

    let event_authority = Address::find_program_address(&[b"__event_authority"], &pump_program).0;

    let accounts = vec![
        AccountMeta::new(user.pubkey(), true),
        AccountMeta::new(uva_pda, false),
        AccountMeta::new_readonly(event_authority, false),
        AccountMeta::new_readonly(pump_program, false),
    ];
    let ix = Instruction { program_id: pump_program, accounts, data: CLOSE_USER_VOLUME_ACCUMULATOR_DISCRIMINATOR.to_vec() };
    let budget_ix = set_compute_unit_limit_ix(200_000 + call_index);
    let blockhash = svm.latest_blockhash();
    let msg = Message::new(&[budget_ix, ix], Some(&user.pubkey()));
    let tx = Transaction::new(&[&user], msg, blockhash);

    println!("=== {label} ===");
    match svm.send_transaction(tx) {
        Ok(meta) => {
            println!("  RESULT: SUCCESS ({} CU)", meta.compute_units_consumed);
            for line in &meta.logs {
                if line.starts_with("Program log:") || line.starts_with("Program data:") {
                    println!("  LOG: {line}");
                }
            }
            let closed_account = svm.get_account(&uva_pda);
            match closed_account {
                Some(acc) if acc.lamports == 0 && acc.data.is_empty() => println!("  user_volume_accumulator: fully closed (0 lamports, 0 data)"),
                Some(acc) => println!("  user_volume_accumulator: NOT fully closed! lamports={} data_len={}", acc.lamports, acc.data.len()),
                None => println!("  user_volume_accumulator: account no longer exists"),
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
}

fn main() {
    let pump_program = pk(PUMP_PROGRAM_ID);
    let mut svm = LiteSVM::new();
    let so_path = format!("{}/../pump-rust-client/artifacts/pump.so", env!("CARGO_MANIFEST_DIR"));
    svm.add_program_from_file(pump_program, &so_path).unwrap_or_else(|e| panic!("load pump.so: {e:?}"));
    svm.set_sysvar::<Clock>(&Clock { unix_timestamp: 1_700_000_000, slot: 100, epoch: 0, leader_schedule_epoch: 0, epoch_start_timestamp: 0 });

    let rent_exempt = svm.minimum_balance_for_rent_exemption(137);

    // Case 1: everything zero -- baseline, should succeed if anything can.
    run_close(
        &mut svm,
        "all fields zero (baseline)",
        0,
        pump_program,
        |user| user_volume_accumulator_data(user, false, 0, 0, 0, 0, false, 0, 0, 0, 0),
        rent_exempt,
    );

    // Case 2: nonzero cashback_earned + stable_cashback_earned only.
    run_close(
        &mut svm,
        "nonzero cashback_earned + stable_cashback_earned",
        1,
        pump_program,
        |user| user_volume_accumulator_data(user, false, 0, 0, 0, 0, false, 5_000, 0, 3_000, 0),
        rent_exempt + 8_000,
    );

    // Case 3: nonzero total_unclaimed_tokens only.
    run_close(
        &mut svm,
        "nonzero total_unclaimed_tokens + needs_claim=true",
        2,
        pump_program,
        |user| user_volume_accumulator_data(user, true, 1_000_000, 0, 0, 0, false, 0, 0, 0, 0),
        rent_exempt,
    );

    // Case 4: everything nonzero simultaneously.
    run_close(
        &mut svm,
        "everything nonzero simultaneously",
        3,
        pump_program,
        |user| user_volume_accumulator_data(user, true, 1_000_000, 500_000, 2_000_000, 1_699_000_000, true, 5_000, 1_000, 3_000, 500),
        rent_exempt + 8_000,
    );
}
