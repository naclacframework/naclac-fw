//! Resolves the two open questions `probe1.rs` left deferred rather than
//! verified:
//! 1. Does the REAL WSOL mint address (not just an arbitrary
//!    no-authority mint) pass the mint-safety check on its own merit, with
//!    an EMPTY `mint_whitelist` (i.e. not via whitelist bypass)? The IDL's
//!    own `WRAPPED_SOL_MINT`/`WRAPPED_SOL_MINT_2022` constants suggest WSOL
//!    might be hardcoded as always-safe rather than merely happening to have
//!    no mint/freeze authority.
//! 2. Is there a real on-chain `message` length limit (`InvalidMessageLength`,
//!    error 6009 exists in the IDL's error table) that a caller must respect,
//!    or is `message` effectively unbounded (up to transaction-size limits)?
//!
//! Both are directly testable against the real dumped `.so`, the same way
//! `probe1.rs` resolved the tip/gross-amount question — no reason to leave
//! either as an assumption when the real bytecode can just answer it.

use litesvm::LiteSVM;
use solana_sdk::{
    account::Account,
    instruction::{AccountMeta, Instruction},
    message::Message,
    pubkey::Pubkey,
    signature::{Keypair, Signer},
    transaction::Transaction,
};
use std::str::FromStr;

const DONATION_RELAY_PROGRAM_ID: &str = "RLAYHr9TRFcKB2ubYQhspcnXiaGpaVzNQvHytt47RZu";
const TOKEN_PROGRAM_ID: &str = "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA";
const ASSOCIATED_TOKEN_PROGRAM_ID: &str = "ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL";
const SYSTEM_PROGRAM_ID: &str = "11111111111111111111111111111111";
const WSOL_MINT_ID: &str = "So11111111111111111111111111111111111111112";

const DONATE_PUBKEY_CONFIG_ID_WITH_PAYER_V1_DISCRIMINATOR: [u8; 8] =
    [120, 217, 57, 241, 135, 104, 139, 184];

const EPOCH_TRACKER_V1_SEED: &[u8] = b"epoch_tracker_v1";
const DEBOUNCER_V1_SEED: &[u8] = b"debouncer_v1";
const MINT_WHITELIST_V1_SEED: &[u8] = b"mint_whitelist_v1";
const MINT_WHITELIST_V1_DISCRIMINATOR: [u8; 8] = [73, 115, 177, 235, 234, 1, 95, 67];

fn pk(s: &str) -> Pubkey {
    Pubkey::from_str(s).unwrap()
}

fn ata_address(owner: &Pubkey, mint: &Pubkey, token_program: &Pubkey) -> Pubkey {
    Pubkey::find_program_address(
        &[owner.as_ref(), token_program.as_ref(), mint.as_ref()],
        &pk(ASSOCIATED_TOKEN_PROGRAM_ID),
    )
    .0
}

fn mint_account_data(decimals: u8) -> Vec<u8> {
    // No mint authority, no freeze authority — real WSOL's own shape.
    let mut data = vec![0u8; 82];
    data[36..44].copy_from_slice(&0u64.to_le_bytes());
    data[44] = decimals;
    data[45] = 1; // is_initialized
    data
}

fn token_account_data(mint: &Pubkey, owner: &Pubkey, amount: u64) -> Vec<u8> {
    let mut data = vec![0u8; 165];
    data[0..32].copy_from_slice(mint.as_ref());
    data[32..64].copy_from_slice(owner.as_ref());
    data[64..72].copy_from_slice(&amount.to_le_bytes());
    data[108] = 1;
    data
}

fn mint_whitelist_v1_account_data(bump: u8, mints: &[Pubkey]) -> Vec<u8> {
    let mut data = Vec::new();
    data.extend_from_slice(&MINT_WHITELIST_V1_DISCRIMINATOR);
    data.push(bump);
    data.extend_from_slice(&(mints.len() as u32).to_le_bytes());
    for m in mints {
        data.extend_from_slice(m.as_ref());
    }
    data
}

/// Runs one `donate_pubkey_config_id_with_payer_v1` scenario in a fresh
/// litesvm instance. `mint` lets the caller pick a specific address (to test
/// the real WSOL address specifically, not just a WSOL-shaped one).
/// `message` lets the caller vary its length. Returns `Ok(())` on success or
/// `Err((custom_code, logs))` on failure.
fn run_scenario(
    scenario_name: &str,
    mint: Pubkey,
    decimals: u8,
    message: &str,
) -> Result<(), (Option<u32>, Vec<String>)> {
    let program_id = pk(DONATION_RELAY_PROGRAM_ID);
    let token_program = pk(TOKEN_PROGRAM_ID);
    let associated_token_program = pk(ASSOCIATED_TOKEN_PROGRAM_ID);
    let system_program = pk(SYSTEM_PROGRAM_ID);

    let mut svm = LiteSVM::new();
    let so_path = format!("{}/artifacts/donation_relay.so", env!("CARGO_MANIFEST_DIR"));
    svm.add_program_from_file(program_id, &so_path)
        .unwrap_or_else(|e| panic!("load donation_relay.so: {e:?}"));

    svm.set_account(
        mint,
        Account {
            lamports: 10_000_000,
            data: mint_account_data(decimals),
            owner: token_program,
            executable: false,
            rent_epoch: 0,
        },
    )
    .unwrap();

    // Empty whitelist — the point is to prove the mint passes on its own merit, not via bypass.
    let (mint_whitelist, mint_whitelist_bump) =
        Pubkey::find_program_address(&[MINT_WHITELIST_V1_SEED], &program_id);
    svm.set_account(
        mint_whitelist,
        Account {
            lamports: 10_000_000,
            data: mint_whitelist_v1_account_data(mint_whitelist_bump, &[]),
            owner: program_id,
            executable: false,
            rent_epoch: 0,
        },
    )
    .unwrap();

    let from = Keypair::new();
    svm.airdrop(&from.pubkey(), 10_000_000_000).unwrap();
    let from_token_account = ata_address(&from.pubkey(), &mint, &token_program);
    const DONATION_AMOUNT: u64 = 1_000_000;
    svm.set_account(
        from_token_account,
        Account {
            lamports: 10_000_000,
            data: token_account_data(&mint, &from.pubkey(), DONATION_AMOUNT * 10),
            owner: token_program,
            executable: false,
            rent_epoch: 0,
        },
    )
    .unwrap();

    let payer = Keypair::new();
    svm.airdrop(&payer.pubkey(), 10_000_000_000).unwrap();

    let config_id = Pubkey::new_unique();
    let credited_to = Pubkey::new_unique();

    let (epoch_tracker, _) = Pubkey::find_program_address(
        &[EPOCH_TRACKER_V1_SEED, config_id.as_ref(), mint.as_ref()],
        &program_id,
    );
    let (debouncer, _) = Pubkey::find_program_address(
        &[DEBOUNCER_V1_SEED, config_id.as_ref(), mint.as_ref()],
        &program_id,
    );
    let debouncer_token_account = ata_address(&debouncer, &mint, &token_program);
    let event_authority = Pubkey::find_program_address(&[b"__event_authority"], &program_id).0;

    let mut data = DONATE_PUBKEY_CONFIG_ID_WITH_PAYER_V1_DISCRIMINATOR.to_vec();
    data.extend_from_slice(&DONATION_AMOUNT.to_le_bytes());
    data.extend_from_slice(config_id.as_ref());
    data.extend_from_slice(&0u16.to_le_bytes()); // tip_bps = 0, irrelevant to this test
    data.extend_from_slice(&(message.len() as u32).to_le_bytes());
    data.extend_from_slice(message.as_bytes());
    data.extend_from_slice(credited_to.as_ref());

    let ix = Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(epoch_tracker, false),
            AccountMeta::new(debouncer, false),
            AccountMeta::new(debouncer_token_account, false),
            AccountMeta::new(mint, false),
            AccountMeta::new(from_token_account, false),
            AccountMeta::new(from.pubkey(), true),
            AccountMeta::new(payer.pubkey(), true),
            AccountMeta::new_readonly(mint_whitelist, false),
            AccountMeta::new_readonly(associated_token_program, false),
            AccountMeta::new_readonly(token_program, false),
            AccountMeta::new_readonly(system_program, false),
            AccountMeta::new_readonly(event_authority, false),
            AccountMeta::new_readonly(program_id, false),
        ],
        data,
    };

    let blockhash = svm.latest_blockhash();
    let msg = Message::new(&[ix], Some(&payer.pubkey()));
    let tx = Transaction::new(&[&payer, &from], msg, blockhash);

    println!("--- Scenario: {scenario_name} (mint={mint}, message_len={}) ---", message.len());
    match svm.send_transaction(tx) {
        Ok(_) => {
            println!("  RESULT: SUCCESS");
            Ok(())
        }
        Err(e) => {
            let code = match e.err {
                solana_sdk::transaction::TransactionError::InstructionError(
                    _,
                    solana_sdk::instruction::InstructionError::Custom(c),
                ) => Some(c),
                _ => None,
            };
            println!("  RESULT: FAILED: {:?} (custom code {:?})", e.err, code);
            for line in &e.meta.logs {
                println!("    {line}");
            }
            Err((code, e.meta.logs))
        }
    }
}

fn main() {
    // --- Question 1: does the REAL WSOL mint address pass on its own merit? ---
    let wsol_mint = pk(WSOL_MINT_ID);
    let wsol_result = run_scenario("real WSOL mint, empty whitelist", wsol_mint, 9, "wsol-test");

    // Also check a WSOL-*shaped* but different-address mint, to isolate
    // "any no-authority mint passes" from "WSOL specifically is hardcoded".
    let shaped_mint = Pubkey::new_unique();
    let shaped_result = run_scenario("WSOL-shaped but different address, empty whitelist", shaped_mint, 9, "shape-test");

    // --- Question 2: is there a real on-chain message length limit? ---
    let short_msg = "a".repeat(32);
    let short_result = run_scenario("short message (32 chars)", Pubkey::new_unique(), 9, &short_msg);

    let medium_msg = "a".repeat(128);
    let medium_result = run_scenario("medium message (128 chars)", Pubkey::new_unique(), 9, &medium_msg);

    let long_msg = "a".repeat(256);
    let long_result = run_scenario("long message (256 chars)", Pubkey::new_unique(), 9, &long_msg);

    let very_long_msg = "a".repeat(600);
    let very_long_result = run_scenario("very long message (600 chars)", Pubkey::new_unique(), 9, &very_long_msg);

    println!("\n=== CONCLUSIONS ===");
    println!(
        "Real WSOL mint (empty whitelist): {}",
        if wsol_result.is_ok() { "PASSES on its own merit" } else { "REJECTED — WSOL is NOT automatically safe" }
    );
    println!(
        "WSOL-shaped different-address mint (empty whitelist): {}",
        if shaped_result.is_ok() { "PASSES (any no-authority mint is safe, not WSOL-specific)" } else { "REJECTED (address-specific check, not shape-based)" }
    );
    for (label, result) in [
        ("32 chars", &short_result),
        ("128 chars", &medium_result),
        ("256 chars", &long_result),
        ("600 chars", &very_long_result),
    ] {
        match result {
            Ok(()) => println!("message length {label}: SUCCEEDED"),
            Err((Some(6009), _)) => println!("message length {label}: FAILED with InvalidMessageLength (6009) — real limit found"),
            Err((code, _)) => println!("message length {label}: FAILED with different error {code:?} (not the length check — likely tx-size limit or something else)"),
        }
    }
}
