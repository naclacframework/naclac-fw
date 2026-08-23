//! Only 10 real instructions write to `Global` at all (confirmed via a
//! proper JSON parse of `pump.json`, not grep): add_quote_mint, initialize,
//! remove_quote_mint, set_params, set_reserved_fee_recipients,
//! set_virtual_quote_reserves, toggle_cashback_enabled, toggle_create_v2,
//! toggle_mayhem_mode, update_buyback_config, update_global_authority.
//! initialize/set_params/update_buyback_config/set_reserved_fee_recipients
//! already ruled out directly. This tests `add_quote_mint` (single arg:
//! `quote_mint: pubkey`) for a hidden remaining-accounts requirement, the
//! same way `set_params` had one the stale IDL didn't show.

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
const ADD_QUOTE_MINT_DISCRIMINATOR: [u8; 8] = [111, 121, 21, 56, 40, 24, 94, 209];
const GLOBAL_DISCRIMINATOR: [u8; 8] = [167, 232, 232, 177, 200, 108, 114, 127];

fn pk(s: &str) -> Pubkey {
    Pubkey::from_str(s).unwrap()
}

fn set_compute_unit_limit_ix(units: u32) -> Instruction {
    let mut data = vec![2u8];
    data.extend_from_slice(&units.to_le_bytes());
    Instruction { program_id: pk("ComputeBudget111111111111111111111111111111"), accounts: vec![], data }
}

fn global_account_data(authority: &Pubkey, buyback_marker: &Pubkey) -> Vec<u8> {
    let mut data = Vec::new();
    data.extend_from_slice(&GLOBAL_DISCRIMINATOR);
    data.push(1);
    data.extend_from_slice(authority.as_ref());
    data.extend_from_slice(&[0u8; 32]); // fee_recipient
    data.extend_from_slice(&[0u8; 8 * 5]);
    data.extend_from_slice(&[0u8; 32]); // withdraw_authority
    data.push(0);
    data.extend_from_slice(&[0u8; 8 * 2]);
    data.extend_from_slice(&[0u8; 32 * 7]); // fee_recipients[7]
    data.extend_from_slice(&[0u8; 32]);
    data.extend_from_slice(&[0u8; 32]);
    data.push(0);
    data.extend_from_slice(&[0u8; 32]);
    data.extend_from_slice(&[0u8; 32]);
    data.push(0);
    data.extend_from_slice(&[0u8; 32 * 7]); // reserved_fee_recipients[7]
    data.push(0);
    for _ in 0..8 {
        data.extend_from_slice(buyback_marker.as_ref()); // buyback_fee_recipients[8], all set to a known marker
    }
    data.extend_from_slice(&0u64.to_le_bytes());
    data.extend_from_slice(&0u64.to_le_bytes());
    data.extend_from_slice(&[0u8; 32]);
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
    let buyback_marker = Pubkey::new_unique();
    println!("buyback_marker (pre-existing, should stay unchanged if add_quote_mint doesn't touch it) = {buyback_marker}\n");

    let (global, _) = Pubkey::find_program_address(&[b"global"], &pump_program);
    svm.set_account(
        global,
        Account { lamports: 10_000_000, data: global_account_data(&authority.pubkey(), &buyback_marker), owner: pump_program, executable: false, rent_epoch: 0 },
    )
    .unwrap();

    let (event_authority, _) = Pubkey::find_program_address(&[b"__event_authority"], &pump_program);
    let new_quote_mint = Pubkey::new_unique();

    let mut data = ADD_QUOTE_MINT_DISCRIMINATOR.to_vec();
    data.extend_from_slice(new_quote_mint.as_ref());

    let ix = Instruction {
        program_id: pump_program,
        accounts: vec![
            AccountMeta::new(global, false),
            AccountMeta::new(authority.pubkey(), true),
            AccountMeta::new_readonly(event_authority, false),
            AccountMeta::new_readonly(pump_program, false),
        ],
        data,
    };

    let budget_ix = set_compute_unit_limit_ix(400_000);
    let blockhash = svm.latest_blockhash();
    let msg = Message::new(&[budget_ix, ix], Some(&authority.pubkey()));
    let tx = Transaction::new(&[&authority], msg, blockhash);

    match svm.send_transaction(tx) {
        Ok(meta) => {
            println!("RESULT: SUCCESS (0 remaining accounts)");
            for line in &meta.logs {
                println!("  LOG: {line}");
            }
            let account = svm.get_account(&global).unwrap();
            let stored = &account.data[741..741 + 256];
            let unchanged = (0..8).all(|i| &stored[i * 32..i * 32 + 32] == buyback_marker.as_ref());
            println!("\nGlobal.buyback_fee_recipients[8] unchanged after add_quote_mint? {unchanged}");
        }
        Err(e) => {
            println!("RESULT: FAILED: {:?}", e.err);
            for line in &e.meta.logs {
                println!("  LOG: {line}");
            }
            println!("\n=> If NotEnoughRemainingAccounts: found a hidden requirement, same pattern as set_params.");
        }
    }
}
