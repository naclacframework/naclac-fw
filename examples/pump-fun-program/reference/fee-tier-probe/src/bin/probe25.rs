//! `initialize` has 0 documented args and only 3 documented accounts
//! (global, user, system_program) — but `set_params` looked exactly this
//! innocuous in its own stale IDL entry too, and turned out to need 8
//! undocumented remaining accounts. This calls real `initialize` on a
//! fresh, uninitialized `global` PDA with 0 remaining accounts first (to
//! see if it errors the same way `set_params` did), then inspects whatever
//! `Global.buyback_fee_recipients` ends up as if it succeeds.

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
const INITIALIZE_DISCRIMINATOR: [u8; 8] = [175, 175, 109, 31, 13, 152, 155, 237];

fn pk(s: &str) -> Pubkey {
    Pubkey::from_str(s).unwrap()
}

fn set_compute_unit_limit_ix(units: u32) -> Instruction {
    let mut data = vec![2u8];
    data.extend_from_slice(&units.to_le_bytes());
    Instruction { program_id: pk("ComputeBudget111111111111111111111111111111"), accounts: vec![], data }
}

fn main() {
    let pump_program = pk(PUMP_PROGRAM_ID);
    let mut svm = LiteSVM::new();
    let so_path = format!("{}/../pump-rust-client/artifacts/pump.so", env!("CARGO_MANIFEST_DIR"));
    svm.add_program_from_file(pump_program, &so_path).unwrap_or_else(|e| panic!("load pump.so: {e:?}"));
    svm.set_sysvar::<Clock>(&Clock { unix_timestamp: 1_700_000_000, slot: 100, epoch: 0, leader_schedule_epoch: 0, epoch_start_timestamp: 0 });

    let user = Keypair::new();
    svm.airdrop(&user.pubkey(), 10_000_000_000).unwrap();

    // Deliberately do NOT pre-set `global` — `initialize` is real `init`,
    // it must create the account itself. Nothing else pre-seeded.
    let (global, _) = Pubkey::find_program_address(&[b"global"], &pump_program);
    println!("global = {global} (fresh, not yet created)\n");

    let data = INITIALIZE_DISCRIMINATOR.to_vec();
    let ix = Instruction {
        program_id: pump_program,
        accounts: vec![
            AccountMeta::new(global, false),
            AccountMeta::new(user.pubkey(), true),
            AccountMeta::new_readonly(pk("11111111111111111111111111111111"), false),
        ],
        data,
    };

    let budget_ix = set_compute_unit_limit_ix(400_000);
    let blockhash = svm.latest_blockhash();
    let msg = Message::new(&[budget_ix, ix], Some(&user.pubkey()));
    let tx = Transaction::new(&[&user], msg, blockhash);

    match svm.send_transaction(tx) {
        Ok(meta) => {
            println!("RESULT: SUCCESS (0 remaining accounts)");
            for line in &meta.logs {
                println!("  LOG: {line}");
            }
            let account = svm.get_account(&global).expect("global should exist now");
            println!("\nGlobal real size after initialize: {} bytes", account.data.len());
            // buyback_fee_recipients[8] at real offset 741, 256 bytes.
            if account.data.len() >= 741 + 256 {
                let stored = &account.data[741..741 + 256];
                let all_zero = stored.iter().all(|&b| b == 0);
                println!("Global.buyback_fee_recipients[8] all-zero after initialize? {all_zero}");
                if !all_zero {
                    for i in 0..8 {
                        let entry = &stored[i * 32..i * 32 + 32];
                        if let Ok(p) = Pubkey::try_from(entry) {
                            println!("  [{i}] {p}");
                        }
                    }
                }
            } else {
                println!("Global account too short to read buyback_fee_recipients (only {} bytes) -- real init-time size may be smaller than the full 1045-byte layout.", account.data.len());
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
