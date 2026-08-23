//! Resolves the open questions left after reading `donation_relay`'s IDL alone
//! (`RLAYHr9TRFcKB2ubYQhspcnXiaGpaVzNQvHytt47RZu-idl.json`) for
//! `donate_pubkey_config_id_with_payer_v1`:
//! 1. Whether `tip_bps` is deducted from `amount` immediately, or purely
//!    recorded for later distribution accounting.
//! 2. Whether `debouncer.total_amount` includes or excludes the tip portion.
//! 3. Which accounts the real instruction actually requires to pre-exist
//!    (`mint_whitelist` almost certainly must, since nothing in this
//!    instruction's own account list creates it) versus which it creates
//!    itself (`epoch_tracker`/`debouncer`/`debouncer_token_account` are
//!    presumably `init_if_needed`, brand-new per `(config_id, mint)` pair).
//!
//! Strategy: load the REAL `donation_relay.so` (dumped from mainnet — see
//! `artifacts/README.md`) into a fresh litesvm instance, fund a real SPL
//! mint + `from`'s token account, inject an empty `MintWhitelistV1` fixture
//! (the only account this instruction doesn't appear to create itself), call
//! the real instruction with a small `tip_bps`, and read back the actual
//! `debouncer`/`epoch_tracker`/`debouncer_token_account` state plus the
//! decoded `DonationMadeV1Event` — no guessing at Rust source, no reliance on
//! doc comments alone.

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

const DONATE_PUBKEY_CONFIG_ID_WITH_PAYER_V1_DISCRIMINATOR: [u8; 8] =
    [120, 217, 57, 241, 135, 104, 139, 184];
const DONATION_MADE_V1_EVENT_DISCRIMINATOR: [u8; 8] = [165, 229, 86, 237, 244, 77, 38, 222];

const EPOCH_TRACKER_V1_SEED: &[u8] = b"epoch_tracker_v1";
const DEBOUNCER_V1_SEED: &[u8] = b"debouncer_v1";
const MINT_WHITELIST_V1_SEED: &[u8] = b"mint_whitelist_v1";
const MINT_WHITELIST_V1_DISCRIMINATOR: [u8; 8] = [73, 115, 177, 235, 234, 1, 95, 67];
const DEBOUNCER_V1_DISCRIMINATOR: [u8; 8] = [137, 137, 192, 77, 110, 184, 189, 28];
const EPOCH_TRACKER_V1_DISCRIMINATOR: [u8; 8] = [130, 12, 50, 226, 170, 109, 160, 155];

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

fn mint_account_data(mint_authority: Option<&Pubkey>, decimals: u8) -> Vec<u8> {
    let mut data = vec![0u8; 82];
    if let Some(auth) = mint_authority {
        data[0..4].copy_from_slice(&1u32.to_le_bytes());
        data[4..36].copy_from_slice(auth.as_ref());
    }
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
    data[108] = 1; // state = Initialized
    data
}

/// Standard Anchor/Borsh account: 8-byte discriminator + fields in the
/// struct's own declared order. `MintWhitelistV1 { bump: u8, mints: Vec<Pubkey> }`
/// — this is well-documented, deterministic Borsh encoding (unlike naclac's
/// own zero-copy component layout, which is a genuinely separate, unverified
/// question this probe does NOT need to answer).
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

fn decode_pubkey(data: &[u8], offset: &mut usize) -> Pubkey {
    let p = Pubkey::try_from(&data[*offset..*offset + 32]).unwrap();
    *offset += 32;
    p
}

fn decode_u8(data: &[u8], offset: &mut usize) -> u8 {
    let v = data[*offset];
    *offset += 1;
    v
}

fn decode_u64(data: &[u8], offset: &mut usize) -> u64 {
    let v = u64::from_le_bytes(data[*offset..*offset + 8].try_into().unwrap());
    *offset += 8;
    v
}

fn decode_u128(data: &[u8], offset: &mut usize) -> u128 {
    let v = u128::from_le_bytes(data[*offset..*offset + 16].try_into().unwrap());
    *offset += 16;
    v
}

fn decode_string(data: &[u8], offset: &mut usize) -> String {
    let len = u32::from_le_bytes(data[*offset..*offset + 4].try_into().unwrap()) as usize;
    *offset += 4;
    let s = String::from_utf8_lossy(&data[*offset..*offset + len]).to_string();
    *offset += len;
    s
}

/// Decodes `DonationMadeV1Event`'s real IDL field layout: daas_event_discriminator
/// (enum, 1 byte), config_id([u8;32]), mint(pubkey), epoch(u128), gross_amount(u64),
/// tip(u64), message(string), credited_to(pubkey).
fn print_donation_made_event(payload: &[u8]) {
    let mut o = 8usize; // skip event discriminator, already checked by caller
    let daas_disc = decode_u8(payload, &mut o);
    let config_id = Pubkey::try_from(&payload[o..o + 32]).unwrap();
    o += 32;
    let mint = decode_pubkey(payload, &mut o);
    let epoch = decode_u128(payload, &mut o);
    let gross_amount = decode_u64(payload, &mut o);
    let tip = decode_u64(payload, &mut o);
    let message = decode_string(payload, &mut o);
    let credited_to = decode_pubkey(payload, &mut o);

    println!("DonationMadeV1Event decoded:");
    println!("  daas_event_discriminator (enum variant) = {daas_disc}");
    println!("  config_id = {config_id}");
    println!("  mint = {mint}");
    println!("  epoch = {epoch}");
    println!("  gross_amount = {gross_amount}");
    println!("  tip = {tip}");
    println!("  message = {message:?}");
    println!("  credited_to = {credited_to}");
}

fn base64_decode(s: &str) -> Vec<u8> {
    fn val(c: u8) -> Option<u8> {
        match c {
            b'A'..=b'Z' => Some(c - b'A'),
            b'a'..=b'z' => Some(c - b'a' + 26),
            b'0'..=b'9' => Some(c - b'0' + 52),
            b'+' => Some(62),
            b'/' => Some(63),
            _ => None,
        }
    }
    let bytes: Vec<u8> = s.bytes().filter_map(val).collect();
    let mut out = Vec::with_capacity(bytes.len() * 3 / 4);
    for chunk in bytes.chunks(4) {
        let n = chunk.len();
        let b0 = chunk[0];
        let b1 = if n > 1 { chunk[1] } else { 0 };
        let b2 = if n > 2 { chunk[2] } else { 0 };
        let b3 = if n > 3 { chunk[3] } else { 0 };
        out.push((b0 << 2) | (b1 >> 4));
        if n > 2 {
            out.push((b1 << 4) | (b2 >> 2));
        }
        if n > 3 {
            out.push((b2 << 6) | b3);
        }
    }
    out
}

fn decode_debouncer_v1(data: &[u8]) {
    if data.len() < 8 || data[0..8] != DEBOUNCER_V1_DISCRIMINATOR {
        println!("  (debouncer account does not match DebouncerV1 discriminator)");
        return;
    }
    let mut o = 8usize;
    let bump = decode_u8(data, &mut o);
    let state = decode_u8(data, &mut o); // enum: 0=Uninitialized, 1=Initialized
    let config_id = Pubkey::try_from(&data[o..o + 32]).unwrap();
    o += 32;
    let mint = decode_pubkey(data, &mut o);
    let total_amount = decode_u64(data, &mut o);
    println!(
        "  DebouncerV1 {{ bump: {bump}, state: {state}, config_id: {config_id}, mint: {mint}, total_amount: {total_amount} }}"
    );
}

fn decode_epoch_tracker_v1(data: &[u8]) {
    if data.len() < 8 || data[0..8] != EPOCH_TRACKER_V1_DISCRIMINATOR {
        println!("  (epoch_tracker account does not match EpochTrackerV1 discriminator)");
        return;
    }
    let mut o = 8usize;
    let bump = decode_u8(data, &mut o);
    let state = decode_u8(data, &mut o);
    let config_id = Pubkey::try_from(&data[o..o + 32]).unwrap();
    o += 32;
    let mint = decode_pubkey(data, &mut o);
    let current_epoch = decode_u128(data, &mut o);
    println!(
        "  EpochTrackerV1 {{ bump: {bump}, state: {state}, config_id: {config_id}, mint: {mint}, current_epoch: {current_epoch} }}"
    );
}

fn main() {
    let program_id = pk(DONATION_RELAY_PROGRAM_ID);
    let token_program = pk(TOKEN_PROGRAM_ID);
    let associated_token_program = pk(ASSOCIATED_TOKEN_PROGRAM_ID);
    let system_program = pk(SYSTEM_PROGRAM_ID);

    let mut svm = LiteSVM::new();
    let so_path = format!("{}/artifacts/donation_relay.so", env!("CARGO_MANIFEST_DIR"));
    svm.add_program_from_file(program_id, &so_path)
        .unwrap_or_else(|e| {
            panic!(
                "load donation_relay.so from {so_path}: {e:?}\n\
                 Dump it first: solana program dump {DONATION_RELAY_PROGRAM_ID} {so_path} --url mainnet-beta"
            )
        });

    // --- mint: fresh SPL mint with NO mint authority and NO freeze authority
    // (matching WSOL's own shape, the mint `crank_donation_fee_pda` actually
    // donates) — a mint with a live mint_authority fails the real program's
    // own `MintNotSafeForDonation` check unless whitelisted, confirmed via
    // this probe's first run.
    let mint = Pubkey::new_unique();
    svm.set_account(
        mint,
        Account {
            lamports: 10_000_000,
            data: mint_account_data(None, 6),
            owner: token_program,
            executable: false,
            rent_epoch: 0,
        },
    )
    .unwrap();

    // --- mint_whitelist: empty singleton fixture — this instruction reads it, doesn't appear to create it ---
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

    // --- from: real funded signer, holds the donated tokens in its own ATA ---
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

    // --- payer: separate real funded signer, per this instruction's own documented purpose ---
    let payer = Keypair::new();
    svm.airdrop(&payer.pubkey(), 10_000_000_000).unwrap();

    let config_id = Pubkey::new_unique();
    let credited_to = Pubkey::new_unique();

    let (epoch_tracker, _epoch_tracker_bump) = Pubkey::find_program_address(
        &[EPOCH_TRACKER_V1_SEED, config_id.as_ref(), mint.as_ref()],
        &program_id,
    );
    let (debouncer, _debouncer_bump) = Pubkey::find_program_address(
        &[DEBOUNCER_V1_SEED, config_id.as_ref(), mint.as_ref()],
        &program_id,
    );
    let debouncer_token_account = ata_address(&debouncer, &mint, &token_program);

    let event_authority = Pubkey::find_program_address(&[b"__event_authority"], &program_id).0;

    const TIP_BPS: u16 = 500; // 5%, arbitrary non-zero value chosen to make tip vs gross_amount unambiguous in the result
    const MESSAGE: &str = "probe1";

    let mut data = DONATE_PUBKEY_CONFIG_ID_WITH_PAYER_V1_DISCRIMINATOR.to_vec();
    data.extend_from_slice(&DONATION_AMOUNT.to_le_bytes());
    data.extend_from_slice(config_id.as_ref());
    data.extend_from_slice(&TIP_BPS.to_le_bytes());
    data.extend_from_slice(&(MESSAGE.len() as u32).to_le_bytes());
    data.extend_from_slice(MESSAGE.as_bytes());
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

    println!("BEFORE: from_token_account balance = {}", DONATION_AMOUNT * 10);
    println!("Donating {DONATION_AMOUNT} (tip_bps={TIP_BPS}) config_id={config_id} mint={mint}");

    let blockhash = svm.latest_blockhash();
    let msg = Message::new(&[ix], Some(&payer.pubkey()));
    let tx = Transaction::new(&[&payer, &from], msg, blockhash);

    let logs: Vec<String> = match svm.send_transaction(tx) {
        Ok(meta) => {
            println!("SUCCESS");
            meta.logs
        }
        Err(e) => {
            println!("FAILED: {:?}", e.err);
            for line in &e.meta.logs {
                println!("  {line}");
            }
            return;
        }
    };
    for line in &logs {
        println!("  {line}");
    }

    println!("\n--- AFTER: on-chain state ---");
    if let Some(acc) = svm.get_account(&debouncer) {
        decode_debouncer_v1(&acc.data);
    } else {
        println!("  debouncer account does not exist after the call");
    }
    if let Some(acc) = svm.get_account(&epoch_tracker) {
        decode_epoch_tracker_v1(&acc.data);
    } else {
        println!("  epoch_tracker account does not exist after the call");
    }
    if let Some(acc) = svm.get_account(&debouncer_token_account) {
        if acc.data.len() >= 72 {
            let amount = u64::from_le_bytes(acc.data[64..72].try_into().unwrap());
            println!("  debouncer_token_account token amount = {amount}");
        }
    } else {
        println!("  debouncer_token_account does not exist after the call");
    }
    if let Some(acc) = svm.get_account(&from_token_account) {
        if acc.data.len() >= 72 {
            let amount = u64::from_le_bytes(acc.data[64..72].try_into().unwrap());
            println!("  from_token_account token amount = {amount} (started at {})", DONATION_AMOUNT * 10);
        }
    }

    println!("\n--- Scanning logs for DonationMadeV1Event ---");
    let mut found_event = false;
    for line in &logs {
        if let Some(b64) = line.strip_prefix("Program data: ") {
            let payload = base64_decode(b64);
            if payload.len() >= 8 && payload[0..8] == DONATION_MADE_V1_EVENT_DISCRIMINATOR {
                found_event = true;
                print_donation_made_event(&payload);
            }
        }
    }
    if !found_event {
        println!("No DonationMadeV1Event found in logs.");
    }
}
