//! Follow-up to `probe_extend_account.rs`, which confirmed the real
//! `pump.so`'s `ExtendAccount` has no authority check at all (a stranger
//! successfully grew someone else's `UserVolumeAccumulator`) and, along the
//! way, revealed the real program's currently-compiled sizes: `Global` =
//! 1045 bytes, `UserVolumeAccumulator` = 137 bytes.
//!
//! This probe tests the remaining open question directly: can `ExtendAccount`
//! shrink an account at all, or is it grow-only by construction (matching
//! its literal name -- "Extend", not "Resize")? Builds each account
//! deliberately *larger* than its real compiled size and calls
//! `ExtendAccount` against it. Three possible outcomes:
//!   - shrinks back down to the real compiled size -> symmetric, same as
//!     naclac's own (pre-guard) behavior -- the refund-theft risk is real
//!     for them too, just currently unreachable because nothing has shrunk.
//!   - stays oversized, logged as a no-op / rejected -> grow-only by
//!     construction, the refund-theft risk never applies to them at all,
//!     regardless of any reserved-bytes strategy.
//!   - errors outright -> some other constraint neither of the above.
//!
//! Usage: cargo run --bin probe_extend_account_shrink

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
const SYSTEM_PROGRAM_ID: &str = "11111111111111111111111111111111";

const EXTEND_ACCOUNT_DISCRIMINATOR: [u8; 8] = [234, 102, 194, 203, 150, 72, 62, 229];
const GLOBAL_DISCRIMINATOR: [u8; 8] = [167, 232, 232, 177, 200, 108, 114, 127];
const USER_VOLUME_ACCUMULATOR_DISCRIMINATOR: [u8; 8] = [86, 255, 112, 14, 102, 53, 154, 250];

// Confirmed empirically in `probe_extend_account.rs`'s output, not guessed:
// probe 1 logged "Account already has the correct size" against a 1045-byte
// Global fixture; probe 2 grew a 40-byte UserVolumeAccumulator to 137 bytes.
const REAL_GLOBAL_SIZE: usize = 1045;
const REAL_UVA_SIZE: usize = 137;
const OVERSIZE_PAD: usize = 64;

fn pk(s: &str) -> Pubkey {
    Pubkey::from_str(s).unwrap()
}

fn set_compute_unit_limit_ix(units: u32) -> Instruction {
    let mut data = vec![2u8];
    data.extend_from_slice(&units.to_le_bytes());
    Instruction { program_id: pk("ComputeBudget111111111111111111111111111111"), accounts: vec![], data }
}

fn oversized_global_data() -> Vec<u8> {
    let mut data = vec![0u8; REAL_GLOBAL_SIZE + OVERSIZE_PAD];
    data[..8].copy_from_slice(&GLOBAL_DISCRIMINATOR);
    data
}

fn oversized_user_volume_accumulator_data(user: &Pubkey) -> Vec<u8> {
    let mut data = vec![0u8; REAL_UVA_SIZE + OVERSIZE_PAD];
    data[..8].copy_from_slice(&USER_VOLUME_ACCUMULATOR_DISCRIMINATOR);
    data[8..40].copy_from_slice(user.as_ref());
    data
}

fn extend_account_ix(pump_program: Pubkey, account: Pubkey, signer: Pubkey) -> Instruction {
    let event_authority = Pubkey::find_program_address(&[b"__event_authority"], &pump_program).0;
    Instruction {
        program_id: pump_program,
        accounts: vec![
            AccountMeta::new(account, false),
            AccountMeta::new(signer, true),
            AccountMeta::new_readonly(pk(SYSTEM_PROGRAM_ID), false),
            AccountMeta::new_readonly(event_authority, false),
            AccountMeta::new_readonly(pump_program, false),
        ],
        data: EXTEND_ACCOUNT_DISCRIMINATOR.to_vec(),
    }
}

fn run_extend(svm: &mut LiteSVM, signer: &Keypair, ix: &Instruction, label: &str) {
    let blockhash = svm.latest_blockhash();
    let msg = Message::new(&[set_compute_unit_limit_ix(200_000), ix.clone()], Some(&signer.pubkey()));
    let tx = Transaction::new(&[signer], msg, blockhash);

    println!("\n========== {label} ==========");
    match svm.send_transaction(tx) {
        Ok(meta) => {
            println!("RESULT: SUCCESS, {} CU", meta.compute_units_consumed);
            for line in &meta.logs {
                println!("  LOG: {line}");
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

    let mut svm = LiteSVM::new();
    let so_path = format!("{}/../pump-rust-client/artifacts/pump.so", env!("CARGO_MANIFEST_DIR"));
    svm.add_program_from_file(pump_program, &so_path).unwrap_or_else(|e| panic!("load pump.so: {e:?}"));
    svm.set_sysvar::<Clock>(&Clock { unix_timestamp: 1_700_000_000, slot: 100, epoch: 0, leader_schedule_epoch: 0, epoch_start_timestamp: 0 });

    // --- Probe A: oversized Global ---
    let (global, _) = Pubkey::find_program_address(&[b"global"], &pump_program);
    let oversized_global = oversized_global_data();
    let oversized_global_len = oversized_global.len();
    svm.set_account(
        global,
        Account { lamports: 50_000_000, data: oversized_global, owner: pump_program, executable: false, rent_epoch: 0 },
    )
    .unwrap();

    let caller_a = Keypair::new();
    svm.airdrop(&caller_a.pubkey(), 5_000_000_000).unwrap();

    println!("Global before: {oversized_global_len} bytes (real compiled size is {REAL_GLOBAL_SIZE})");
    run_extend(
        &mut svm,
        &caller_a,
        &extend_account_ix(pump_program, global, caller_a.pubkey()),
        "PROBE A: ExtendAccount against an oversized Global",
    );
    let global_after = svm.get_account(&global).map(|a| a.data.len());
    println!("Global after: {global_after:?} bytes (was {oversized_global_len}, real compiled size {REAL_GLOBAL_SIZE})");

    // --- Probe B: oversized UserVolumeAccumulator ---
    let user = Keypair::new();
    let (user_volume_accumulator, _) =
        Pubkey::find_program_address(&[b"user_volume_accumulator", user.pubkey().as_ref()], &pump_program);
    let oversized_uva = oversized_user_volume_accumulator_data(&user.pubkey());
    let oversized_uva_len = oversized_uva.len();
    svm.set_account(
        user_volume_accumulator,
        Account { lamports: 50_000_000, data: oversized_uva, owner: pump_program, executable: false, rent_epoch: 0 },
    )
    .unwrap();

    let caller_b = Keypair::new();
    svm.airdrop(&caller_b.pubkey(), 5_000_000_000).unwrap();

    println!("\nUserVolumeAccumulator before: {oversized_uva_len} bytes (real compiled size is {REAL_UVA_SIZE})");
    run_extend(
        &mut svm,
        &caller_b,
        &extend_account_ix(pump_program, user_volume_accumulator, caller_b.pubkey()),
        "PROBE B: ExtendAccount against an oversized UserVolumeAccumulator",
    );
    let uva_after = svm.get_account(&user_volume_accumulator).map(|a| a.data.len());
    println!("UserVolumeAccumulator after: {uva_after:?} bytes (was {oversized_uva_len}, real compiled size {REAL_UVA_SIZE})");

    println!("\nCONCLUSION: if either account shrank back down to its real compiled");
    println!("size, ExtendAccount is symmetric (grow AND shrink) on the real program too");
    println!("-- the refund-theft risk applies to them identically, just unreachable");
    println!("today because nothing has actually shrunk yet. If both stayed oversized");
    println!("(no-op or rejected), ExtendAccount is grow-only by construction on the real");
    println!("program, and the refund-theft risk never applies to it regardless of any");
    println!("reserved-bytes strategy.");
}
