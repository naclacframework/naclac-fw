//! Binary-searches the exact `message` length boundary `probe2.rs` bracketed
//! (128 chars succeeded, 256 chars failed with `InvalidMessageLength`/6009)
//! against the real dumped `.so`, so the naclac reimplementation can enforce
//! the real limit exactly rather than picking an arbitrary safe-looking round
//! number.

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
    let mut data = vec![0u8; 82];
    data[36..44].copy_from_slice(&0u64.to_le_bytes());
    data[44] = decimals;
    data[45] = 1;
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

fn mint_whitelist_v1_account_data(bump: u8) -> Vec<u8> {
    let mut data = Vec::new();
    data.extend_from_slice(&MINT_WHITELIST_V1_DISCRIMINATOR);
    data.push(bump);
    data.extend_from_slice(&0u32.to_le_bytes());
    data
}

/// Returns `true` if a `message` of this length succeeds (using a fresh
/// `(config_id, mint)` pair and a fresh litesvm instance each call, so
/// repeated calls in the same run never collide).
fn message_length_succeeds(len: usize) -> bool {
    let program_id = pk(DONATION_RELAY_PROGRAM_ID);
    let token_program = pk(TOKEN_PROGRAM_ID);
    let associated_token_program = pk(ASSOCIATED_TOKEN_PROGRAM_ID);
    let system_program = pk(SYSTEM_PROGRAM_ID);
    let mint = pk(WSOL_MINT_ID);

    let mut svm = LiteSVM::new();
    let so_path = format!("{}/artifacts/donation_relay.so", env!("CARGO_MANIFEST_DIR"));
    svm.add_program_from_file(program_id, &so_path)
        .unwrap_or_else(|e| panic!("load donation_relay.so: {e:?}"));

    svm.set_account(
        mint,
        Account {
            lamports: 10_000_000,
            data: mint_account_data(9),
            owner: token_program,
            executable: false,
            rent_epoch: 0,
        },
    )
    .unwrap();

    let (mint_whitelist, mint_whitelist_bump) =
        Pubkey::find_program_address(&[MINT_WHITELIST_V1_SEED], &program_id);
    svm.set_account(
        mint_whitelist,
        Account {
            lamports: 10_000_000,
            data: mint_whitelist_v1_account_data(mint_whitelist_bump),
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

    let message = "a".repeat(len);
    let mut data = DONATE_PUBKEY_CONFIG_ID_WITH_PAYER_V1_DISCRIMINATOR.to_vec();
    data.extend_from_slice(&DONATION_AMOUNT.to_le_bytes());
    data.extend_from_slice(config_id.as_ref());
    data.extend_from_slice(&0u16.to_le_bytes());
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
    svm.send_transaction(tx).is_ok()
}

fn main() {
    let mut low_ok = 128usize; // known-good from probe2
    let mut high_bad = 256usize; // known-bad from probe2

    println!("Binary-searching the exact message length boundary between {low_ok} (ok) and {high_bad} (fails)...");

    while high_bad - low_ok > 1 {
        let mid = (low_ok + high_bad) / 2;
        let ok = message_length_succeeds(mid);
        println!("  length {mid}: {}", if ok { "OK" } else { "FAILS" });
        if ok {
            low_ok = mid;
        } else {
            high_bad = mid;
        }
    }

    println!("\n=== CONCLUSION ===");
    println!("Maximum allowed `message` length: {low_ok} bytes (length {high_bad} fails with InvalidMessageLength).");
}
