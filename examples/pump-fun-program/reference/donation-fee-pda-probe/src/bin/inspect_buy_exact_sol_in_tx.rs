//! Same technique as `inspect_buy_tx.rs`, applied to a real `buy_exact_sol_in`
//! transaction — settles `bonding-curve-05-batch1-v2-instructions.md`'s open
//! fee-CPI-circularity question directly from real chain data instead of
//! guessing local litesvm fixtures (which repeatedly hit a
//! reserve/scale-independent `BuyZeroAmount` at `buy.rs:56` for reasons not
//! yet understood — see that doc).
//!
//! For the first real `buy_exact_sol_in` found among recent `pump` program
//! signatures: prints the full account list, the raw instruction data
//! (`spendable_sol_in`/`min_tokens_out`/`track_volume`, decoded), and a full
//! dump of the transaction's inner-instruction trace (which contains the
//! nested `pump_fees::get_fees` CPI's own instruction data — the real
//! `trade_size_lamports` argument `pump.so` actually passed, readable
//! directly from the dump rather than inferred).
//!
//! Usage: cargo run --bin inspect_buy_exact_sol_in_tx [-- <rpc-url>]
//!
//! Paginates back through signature history (up to `MAX_PAGES` pages of
//! `PAGE_SIZE` each, `NUM_WORKERS` parallel fetchers per page — sequential
//! fetching at this scale was already confirmed impractically slow against
//! public RPC, see `find_buyback_setter.rs`) since `buy_exact_sol_in` is a
//! newer/rarer instruction than classic `buy` and may not appear in just the
//! most recent page.

use solana_program::pubkey::Pubkey;
use solana_rpc_client::rpc_client::{GetConfirmedSignaturesForAddress2Config, RpcClient};
use solana_rpc_client_api::config::RpcTransactionConfig;
use solana_transaction_status_client_types::{EncodedTransaction, UiMessage, UiTransactionEncoding};
use std::str::FromStr;
use std::sync::{Arc, Mutex};
use std::time::Duration;

const PUMP_PROGRAM_ID: &str = "6EF8rrecthR5Dkzon8Nwu78hRvfCKubJ14M5uBEwF6P";
const BUY_EXACT_SOL_IN_DISCRIMINATOR: [u8; 8] = [56, 252, 116, 8, 158, 223, 205, 95];

const PAGE_SIZE: usize = 1000;
const MAX_PAGES: usize = 30;
const NUM_WORKERS: usize = 16;

fn pk(s: &str) -> Pubkey {
    Pubkey::from_str(s).unwrap()
}

/// Returns true if this signature's transaction (a) actually succeeded
/// (`meta.err.is_none()` — a failed transaction is fully rolled back, so it
/// won't have recorded any real inner-instruction/CPI trace even if it
/// contains a `buy_exact_sol_in` instruction that would otherwise have
/// worked) and (b) contains a `buy_exact_sol_in` instruction.
fn signature_has_successful_buy_exact_sol_in(client: &RpcClient, sig_str: &str) -> bool {
    let Ok(signature) = solana_signature::Signature::from_str(sig_str) else { return false };
    let tx = match client.get_transaction_with_config(
        &signature,
        RpcTransactionConfig { encoding: Some(UiTransactionEncoding::Json), commitment: None, max_supported_transaction_version: Some(0) },
    ) {
        Ok(tx) => tx,
        Err(_) => return false,
    };
    let Some(meta) = &tx.transaction.meta else { return false };
    if meta.err.is_some() {
        return false;
    }
    let EncodedTransaction::Json(ui_tx) = &tx.transaction.transaction else { return false };
    let UiMessage::Raw(raw_msg) = &ui_tx.message else { return false };
    for ix in &raw_msg.instructions {
        if let Ok(data) = bs58::decode(&ix.data).into_vec() {
            if data.len() >= 8 && data[0..8] == BUY_EXACT_SOL_IN_DISCRIMINATOR {
                return true;
            }
        }
    }
    false
}

/// Fully fetches + prints one already-confirmed `buy_exact_sol_in` transaction.
fn print_full_details(client: &RpcClient, sig_str: &str) {
    let signature = solana_signature::Signature::from_str(sig_str).expect("valid signature");
    let mut attempts = 0;
    let tx = loop {
        match client.get_transaction_with_config(
            &signature,
            RpcTransactionConfig { encoding: Some(UiTransactionEncoding::Json), commitment: None, max_supported_transaction_version: Some(0) },
        ) {
            Ok(tx) => break tx,
            Err(e) => {
                attempts += 1;
                if attempts >= 5 {
                    panic!("re-fetch of already-confirmed tx {sig_str} failed after {attempts} attempts: {e:?}");
                }
                println!("  (transient RPC error on attempt {attempts}, retrying: {e:?})");
                std::thread::sleep(Duration::from_millis(1000));
            }
        }
    };

    let EncodedTransaction::Json(ui_tx) = &tx.transaction.transaction else { panic!("not JSON encoded") };
    let UiMessage::Raw(raw_msg) = &ui_tx.message else { panic!("not raw message") };

    let mut all_keys: Vec<String> = raw_msg.account_keys.clone();
    let meta = tx.transaction.meta.as_ref().expect("meta missing");
    if let solana_transaction_status_client_types::option_serializer::OptionSerializer::Some(loaded) = &meta.loaded_addresses {
        all_keys.extend(loaded.writable.iter().cloned());
        all_keys.extend(loaded.readonly.iter().cloned());
    }

    for ix in &raw_msg.instructions {
        let Ok(data) = bs58::decode(&ix.data).into_vec() else { continue };
        if data.len() >= 8 && data[0..8] == BUY_EXACT_SOL_IN_DISCRIMINATOR {
            println!("\n=== Found `buy_exact_sol_in` in tx {sig_str} ===");
            println!("Instruction data ({} bytes): {:02x?}", data.len(), data);

            if data.len() >= 8 + 8 + 8 + 1 {
                let spendable_sol_in = u64::from_le_bytes(data[8..16].try_into().unwrap());
                let min_tokens_out = u64::from_le_bytes(data[16..24].try_into().unwrap());
                let track_volume = data[24];
                println!("  spendable_sol_in = {spendable_sol_in}");
                println!("  min_tokens_out = {min_tokens_out}");
                println!("  track_volume = {track_volume}");
            }

            println!("\nAccounts ({}):", ix.accounts.len());
            for (i, &account_index) in ix.accounts.iter().enumerate() {
                let idx = account_index as usize;
                let key_str = all_keys.get(idx).cloned().unwrap_or_default();
                println!("  [{i}] index={idx} key={key_str}");
            }

            // Nested get_fees CPI lives in here somewhere — full debug dump
            // rather than pattern-matching specific enum variants this crate
            // hasn't decoded before (avoids guessing at exact type names
            // offline). Look for `pfeeUxB6...` (pump_fees) and a data array
            // starting `[231, 37, 126, 85, 207, 91, 63, 52` (the real
            // `get_fees` discriminator) in the printed output below. Layout
            // after the 8-byte discriminator: is_pump_pool(1 byte) at [8],
            // market_cap_lamports(u128, little-endian) at [9..25),
            // trade_size_lamports(u64, little-endian) at [25..33),
            // is_new_quote_mint(1 byte) at [33].
            println!("\nmeta.err = {:?}", meta.err);
            println!("\nFull meta.inner_instructions (raw debug dump):");
            println!("{:#?}", meta.inner_instructions);
            if let solana_transaction_status_client_types::option_serializer::OptionSerializer::Some(logs) = &meta.log_messages {
                println!("\nlog_messages:");
                for l in logs {
                    println!("  {l}");
                }
            } else {
                println!("\n(no log_messages)");
            }
            return;
        }
    }
    println!("WARNING: re-fetch of {sig_str} no longer shows buy_exact_sol_in?!");
}

fn main() {
    let rpc_url = std::env::args().nth(1).unwrap_or_else(|| "https://api.mainnet-beta.solana.com".to_string());
    let pump_program = pk(PUMP_PROGRAM_ID);
    let listing_client = RpcClient::new_with_timeout(rpc_url.clone(), Duration::from_secs(60));

    let mut before: Option<solana_signature::Signature> = None;
    let mut scanned = 0usize;
    let found: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));

    for page in 0..MAX_PAGES {
        let sigs = match listing_client.get_signatures_for_address_with_config(
            &pump_program,
            GetConfirmedSignaturesForAddress2Config { before, until: None, limit: Some(PAGE_SIZE), commitment: None },
        ) {
            Ok(s) => s,
            Err(e) => {
                println!("RPC error on page {page}: {e:?} -- stopping.");
                break;
            }
        };
        if sigs.is_empty() {
            println!("No more signatures (stopped at page {page}).");
            break;
        }
        scanned += sigs.len();
        println!("Page {page}: scanning {} signature(s) (total scanned so far: {scanned})...", sigs.len());

        let sig_strings: Vec<String> = sigs.iter().map(|s| s.signature.clone()).collect();
        let chunks: Vec<Vec<String>> = sig_strings.chunks(sig_strings.len().div_ceil(NUM_WORKERS).max(1)).map(|c| c.to_vec()).collect();

        std::thread::scope(|scope| {
            for chunk in chunks {
                let rpc_url = rpc_url.clone();
                let found = Arc::clone(&found);
                scope.spawn(move || {
                    let client = RpcClient::new_with_timeout(rpc_url, Duration::from_secs(60));
                    for sig_str in &chunk {
                        if found.lock().unwrap().is_some() {
                            return;
                        }
                        if signature_has_successful_buy_exact_sol_in(&client, sig_str) {
                            let mut f = found.lock().unwrap();
                            if f.is_none() {
                                *f = Some(sig_str.clone());
                            }
                            return;
                        }
                    }
                });
            }
        });

        if let Some(sig_str) = found.lock().unwrap().clone() {
            print_full_details(&listing_client, &sig_str);
            return;
        }

        before = sigs.last().map(|s| solana_signature::Signature::from_str(&s.signature).unwrap());
    }

    println!("\nNo `buy_exact_sol_in` instruction found across {scanned} scanned signature(s).");
}
