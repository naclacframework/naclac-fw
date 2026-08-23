//! Probes the real, deployed `pump.so`'s `ExtendAccount` instruction for the
//! authority question raised while building naclac's own copy of it: is it
//! genuinely permissionless (any signer, against any account it owns), or
//! is there an internal authority/relation check not visible in the IDL
//! (zero args, no dedicated authority account slot)?
//!
//! Two calls, both against the real compiled binary in LiteSVM (no real
//! funds, no real network):
//!   1. `Global`, deliberately built undersized (one `whitelisted_quote_mints`
//!      slot short of the real, currently-live two-slot layout), extended by
//!      a throwaway signer with no relation to it at all.
//!   2. A `UserVolumeAccumulator` PDA seeded to `victim`, also deliberately
//!      undersized, extended by a completely different `attacker` signer who
//!      pays the rent themselves -- the direct test of "can a stranger grow
//!      someone else's per-user account."
//!
//! Usage: cargo run --bin probe_extend_account

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

fn pk(s: &str) -> Pubkey {
    Pubkey::from_str(s).unwrap()
}

fn set_compute_unit_limit_ix(units: u32) -> Instruction {
    let mut data = vec![2u8];
    data.extend_from_slice(&units.to_le_bytes());
    Instruction { program_id: pk("ComputeBudget111111111111111111111111111111"), accounts: vec![], data }
}

/// One `Address` short of the real, currently-live two-slot
/// `whitelisted_quote_mints` layout -- same undersized shape used to prove
/// naclac's own `extend_account` grows a real pre-extend `Global`.
fn old_one_slot_global_account_data(authority: &Pubkey) -> Vec<u8> {
    let mut data = Vec::new();
    data.extend_from_slice(&GLOBAL_DISCRIMINATOR);
    data.push(1); // initialized
    data.extend_from_slice(authority.as_ref());
    data.extend_from_slice(&[0u8; 32]); // fee_recipient
    data.extend_from_slice(&[0u8; 8 * 5]); // 5 u64 reserve/fee fields
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
    data.extend_from_slice(&[0u8; 32]); // whitelisted_quote_mints[0] -- ONLY one slot
    data
}

/// Deliberately undersized `UserVolumeAccumulator` fixture -- just the
/// discriminator plus a handful of zeroed bytes, well short of any real
/// compiled size. Content doesn't need to match a real historical layout;
/// this only tests the resize mechanism and its authority (or lack of it).
fn undersized_user_volume_accumulator_data(user: &Pubkey) -> Vec<u8> {
    let mut data = Vec::new();
    data.extend_from_slice(&USER_VOLUME_ACCUMULATOR_DISCRIMINATOR);
    data.extend_from_slice(user.as_ref()); // user
    data
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

fn main() {
    let pump_program = pk(PUMP_PROGRAM_ID);

    let mut svm = LiteSVM::new();
    let so_path = format!("{}/../pump-rust-client/artifacts/pump.so", env!("CARGO_MANIFEST_DIR"));
    svm.add_program_from_file(pump_program, &so_path).unwrap_or_else(|e| panic!("load pump.so: {e:?}"));
    svm.set_sysvar::<Clock>(&Clock { unix_timestamp: 1_700_000_000, slot: 100, epoch: 0, leader_schedule_epoch: 0, epoch_start_timestamp: 0 });

    // --- Probe 1: Global, extended by a stranger with no relation to it ---
    let global_authority = Keypair::new(); // whoever "owns" Global, per the fixture
    let (global, _) = Pubkey::find_program_address(&[b"global"], &pump_program);
    let old_global_data = old_one_slot_global_account_data(&global_authority.pubkey());
    let old_global_len = old_global_data.len();
    svm.set_account(
        global,
        Account { lamports: 10_000_000_000, data: old_global_data, owner: pump_program, executable: false, rent_epoch: 0 },
    )
    .unwrap();

    let stranger = Keypair::new();
    svm.airdrop(&stranger.pubkey(), 5_000_000_000).unwrap();

    println!("Global before: {old_global_len} bytes");
    run_extend(
        &mut svm,
        &stranger,
        &extend_account_ix(pump_program, global, stranger.pubkey()),
        "PROBE 1: stranger extends undersized Global (no relation to it at all)",
    );
    let global_after = svm.get_account(&global).map(|a| a.data.len());
    println!("Global after: {global_after:?} bytes (was {old_global_len})");

    // --- Probe 2: victim's UserVolumeAccumulator, extended by an attacker ---
    let victim = Keypair::new();
    let (user_volume_accumulator, _) =
        Pubkey::find_program_address(&[b"user_volume_accumulator", victim.pubkey().as_ref()], &pump_program);
    let old_uva_data = undersized_user_volume_accumulator_data(&victim.pubkey());
    let old_uva_len = old_uva_data.len();
    // Generously over-funded on purpose -- probe 2 already proved (previous
    // run) that the real program's own logic completes fully for a stranger
    // (System transfer CPI + event emission both succeeded); the only
    // failure was Solana's post-instruction rent-exempt balance invariant
    // tripping on an under-calibrated starting balance. Funding well above
    // any plausible minimum_balance for this account's grown size removes
    // that entirely, so the transaction's real outcome (program-level
    // accept/reject) is visible without a runtime-level false negative.
    svm.set_account(
        user_volume_accumulator,
        Account { lamports: 50_000_000, data: old_uva_data, owner: pump_program, executable: false, rent_epoch: 0 },
    )
    .unwrap();

    let attacker = Keypair::new();
    svm.airdrop(&attacker.pubkey(), 5_000_000_000).unwrap();

    println!("\nvictim: {}", victim.pubkey());
    println!("attacker: {}", attacker.pubkey());
    println!("UserVolumeAccumulator before: {old_uva_len} bytes, funded by fixture only");
    run_extend(
        &mut svm,
        &attacker,
        &extend_account_ix(pump_program, user_volume_accumulator, attacker.pubkey()),
        "PROBE 2: attacker extends victim's UserVolumeAccumulator, paying the rent themselves",
    );
    let uva_after = svm.get_account(&user_volume_accumulator);
    println!(
        "UserVolumeAccumulator after: {:?} bytes (was {old_uva_len})",
        uva_after.as_ref().map(|a| a.data.len())
    );

    println!("\nCONCLUSION: SUCCESS on either probe with no error log referencing an");
    println!("authority/owner/creator mismatch confirms the real program's ExtendAccount");
    println!("has no authority check at all -- any signer, against any account it owns,");
    println!("exactly like naclac's own version. A FAILED result with an authority-shaped");
    println!("error would mean the real program checks something (e.g. a field inside the");
    println!("account itself) that naclac's version currently does not.");
}
