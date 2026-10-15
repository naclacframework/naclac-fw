//! Settles empirically what real `pump.so`'s `sync_user_volume_accumulator`
//! actually computes. The real event
//! (`SyncUserVolumeAccumulatorEvent{user, total_claimed_tokens_before,
//! total_claimed_tokens_after, timestamp}`) implies it recomputes
//! `total_claimed_tokens` from something -- no IDL doc, reference-client
//! usage, or prior probe in this repo explains what. Hypothesis under test:
//! a day-indexed pro-rata token-incentive formula against
//! `GlobalVolumeAccumulator.total_token_supply[i]`/`sol_volumes[i]`, keyed by
//! `UserVolumeAccumulator.current_sol_volume` for the day(s) elapsed.
//!
//! Real accounts (`pump-public-docs/idl/pump.json`): `user` (plain, no
//! signer at all -- fully permissionless, callable by anyone for anyone),
//! `global_volume_accumulator` (pda, singleton, read-only),
//! `user_volume_accumulator` (mut, pda=[seed, user]), `event_authority`,
//! `program`.

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
const SYNC_USER_VOLUME_ACCUMULATOR_DISCRIMINATOR: [u8; 8] = [86, 31, 192, 87, 163, 87, 79, 238];
const USER_VOLUME_ACCUMULATOR_DISCRIMINATOR: [u8; 8] = [86, 255, 112, 14, 102, 53, 154, 250];
const GLOBAL_VOLUME_ACCUMULATOR_DISCRIMINATOR: [u8; 8] = [202, 42, 246, 43, 142, 190, 30, 255];

fn pk(s: &str) -> Address {
    Address::from_str(s).unwrap()
}

fn set_compute_unit_limit_ix(units: u32) -> Instruction {
    let mut data = vec![2u8];
    data.extend_from_slice(&units.to_le_bytes());
    Instruction { program_id: pk("ComputeBudget111111111111111111111111111111"), accounts: vec![], data }
}

#[allow(clippy::too_many_arguments)]
fn user_volume_accumulator_data(
    user: &Address,
    needs_claim: bool,
    total_unclaimed_tokens: u64,
    total_claimed_tokens: u64,
    current_sol_volume: u64,
    last_update_timestamp: i64,
    has_total_claimed_tokens: bool,
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
    data.extend_from_slice(&0u64.to_le_bytes()); // cashback_earned
    data.extend_from_slice(&0u64.to_le_bytes()); // total_cashback_claimed
    data.extend_from_slice(&0u64.to_le_bytes()); // stable_cashback_earned
    data.extend_from_slice(&0u64.to_le_bytes()); // total_stable_cashback_claimed
    data.resize(137, 0);
    data
}

fn global_volume_accumulator_data(
    start_time: i64,
    end_time: i64,
    seconds_in_a_day: i64,
    mint: &Address,
    total_token_supply: &[u64; 30],
    sol_volumes: &[u64; 30],
) -> Vec<u8> {
    let mut data = Vec::new();
    data.extend_from_slice(&GLOBAL_VOLUME_ACCUMULATOR_DISCRIMINATOR);
    data.extend_from_slice(&start_time.to_le_bytes());
    data.extend_from_slice(&end_time.to_le_bytes());
    data.extend_from_slice(&seconds_in_a_day.to_le_bytes());
    data.extend_from_slice(mint.as_ref());
    for v in total_token_supply {
        data.extend_from_slice(&v.to_le_bytes());
    }
    for v in sol_volumes {
        data.extend_from_slice(&v.to_le_bytes());
    }
    data
}

#[allow(clippy::too_many_arguments)]
fn run_sync(
    svm: &mut LiteSVM,
    label: &str,
    call_index: u32,
    pump_program: Address,
    global_volume_accumulator_data_bytes: Vec<u8>,
    user: &Address,
    uva_data: Vec<u8>,
    now: i64,
) {
    let payer = Keypair::new();
    svm.airdrop(&payer.pubkey(), 10_000_000_000).unwrap();
    svm.set_sysvar::<Clock>(&Clock { unix_timestamp: now, slot: 100, epoch: 0, leader_schedule_epoch: 0, epoch_start_timestamp: 0 });

    let (gva_pda, _) = Address::find_program_address(&[b"global_volume_accumulator"], &pump_program);
    svm.set_account(gva_pda, Account { lamports: 10_000_000, data: global_volume_accumulator_data_bytes, owner: pump_program, executable: false, rent_epoch: 0 }).unwrap();

    let (uva_pda, _) = Address::find_program_address(&[b"user_volume_accumulator", user.as_ref()], &pump_program);
    svm.set_account(uva_pda, Account { lamports: 10_000_000, data: uva_data, owner: pump_program, executable: false, rent_epoch: 0 }).unwrap();

    let event_authority = Address::find_program_address(&[b"__event_authority"], &pump_program).0;

    let accounts = vec![
        AccountMeta::new_readonly(*user, false),
        AccountMeta::new_readonly(gva_pda, false),
        AccountMeta::new(uva_pda, false),
        AccountMeta::new_readonly(event_authority, false),
        AccountMeta::new_readonly(pump_program, false),
    ];
    let ix = Instruction { program_id: pump_program, accounts, data: SYNC_USER_VOLUME_ACCUMULATOR_DISCRIMINATOR.to_vec() };
    let budget_ix = set_compute_unit_limit_ix(300_000 + call_index);
    let blockhash = svm.latest_blockhash();
    let msg = Message::new(&[budget_ix, ix], Some(&payer.pubkey()));
    let tx = Transaction::new(&[&payer], msg, blockhash);

    println!("=== {label} (now={now}) ===");
    match svm.send_transaction(tx) {
        Ok(meta) => {
            println!("  RESULT: SUCCESS ({} CU)", meta.compute_units_consumed);
            for line in &meta.logs {
                if line.starts_with("Program log:") || line.starts_with("Program data:") {
                    println!("  LOG: {line}");
                }
            }
            let after = svm.get_account(&uva_pda).unwrap();
            // total_claimed_tokens sits at offset 8(disc)+32(user)+1(needs_claim)+8(total_unclaimed_tokens) = 49
            let total_claimed_after = u64::from_le_bytes(after.data[49..57].try_into().unwrap());
            let current_sol_volume_after = u64::from_le_bytes(after.data[57..65].try_into().unwrap());
            let has_total_claimed_after = after.data[73];
            println!("  total_claimed_tokens after = {total_claimed_after}");
            println!("  current_sol_volume after = {current_sol_volume_after}");
            println!("  has_total_claimed_tokens after = {has_total_claimed_after}");
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

    let mint = Address::new_unique();
    let seconds_in_a_day: i64 = 86_400;
    let start_time: i64 = 1_700_000_000;
    let end_time: i64 = start_time + seconds_in_a_day * 3; // 3-day window

    let mut total_token_supply = [0u64; 30];
    let mut sol_volumes = [0u64; 30];
    total_token_supply[0] = 1_000_000;
    sol_volumes[0] = 500_000;
    total_token_supply[1] = 2_000_000;
    sol_volumes[1] = 400_000;

    let gva_data = global_volume_accumulator_data(start_time, end_time, seconds_in_a_day, &mint, &total_token_supply, &sol_volumes);

    // Case 1: clock inside day 0, user has half of day 0's sol_volume.
    {
        let user = Address::new_unique();
        let uva = user_volume_accumulator_data(&user, false, 0, 0, 250_000, 0, false);
        run_sync(&mut svm, "mid-day-0, user_sol_volume=250_000 (half of day 0)", 0, pump_program, gva_data.clone(), &user, uva, start_time + 1_000);
    }

    // Case 2: clock just after day 0 ends, into day 1.
    {
        let user = Address::new_unique();
        let uva = user_volume_accumulator_data(&user, false, 0, 0, 250_000, start_time, false);
        run_sync(&mut svm, "start of day 1, user_sol_volume=250_000 (day 0's, unclaimed)", 1, pump_program, gva_data.clone(), &user, uva, start_time + seconds_in_a_day + 10);
    }

    // Case 3: clock after the whole window ends.
    {
        let user = Address::new_unique();
        let uva = user_volume_accumulator_data(&user, false, 0, 0, 250_000, start_time, false);
        run_sync(&mut svm, "after window ends, user_sol_volume=250_000", 2, pump_program, gva_data.clone(), &user, uva, end_time + seconds_in_a_day);
    }

    // Case 4: already has_total_claimed_tokens=true, nonzero total_claimed_tokens already -- does sync touch it again?
    {
        let user = Address::new_unique();
        let uva = user_volume_accumulator_data(&user, false, 0, 777_777, 250_000, start_time, true);
        run_sync(&mut svm, "has_total_claimed_tokens already true, total_claimed_tokens=777_777", 3, pump_program, gva_data.clone(), &user, uva, end_time + seconds_in_a_day);
    }

    // Case 5: current_sol_volume = 0 (never traded).
    {
        let user = Address::new_unique();
        let uva = user_volume_accumulator_data(&user, false, 0, 0, 0, 0, false);
        run_sync(&mut svm, "current_sol_volume=0", 4, pump_program, gva_data.clone(), &user, uva, start_time + 1_000);
    }

    // Case 6: same as case 2 (start of day 1, unclaimed day-0 volume) but
    // with needs_claim=true -- every prior case left this false and all of
    // them logged "No changes in accumulator". Testing whether this flag is
    // the actual gate.
    {
        let user = Address::new_unique();
        let uva = user_volume_accumulator_data(&user, true, 0, 0, 250_000, start_time, false);
        run_sync(&mut svm, "needs_claim=true, start of day 1, user_sol_volume=250_000", 5, pump_program, gva_data.clone(), &user, uva, start_time + seconds_in_a_day + 10);
    }

    // Case 7: needs_claim=true, mid-day-0 (not yet elapsed) -- does it still
    // no-op if the day itself hasn't finished, even with needs_claim set?
    {
        let user = Address::new_unique();
        let uva = user_volume_accumulator_data(&user, true, 0, 0, 250_000, start_time, false);
        run_sync(&mut svm, "needs_claim=true, mid-day-0", 6, pump_program, gva_data.clone(), &user, uva, start_time + 1_000);
    }
}
