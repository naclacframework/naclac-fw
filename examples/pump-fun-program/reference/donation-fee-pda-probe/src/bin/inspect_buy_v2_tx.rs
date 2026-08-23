//! Fetches and decodes a real, successful `buy_v2` transaction, specifically
//! looking for one on a NON-SOL quote mint (e.g. USDC), to settle genuine
//! open questions about `buy_v2`/`sell_v2` that no doc resolves:
//!   1. Does `sharing_config` get read/CPI'd, or is it pure shape-parity?
//!   2. For a non-SOL quote mint, does cashback route through
//!      `associated_user_volume_accumulator` (a quote-mint ATA) or still
//!      through native lamports on `user_volume_accumulator` itself
//!      (as `PUMP_CASHBACK_README.md` claims is universal for the bonding
//!      curve program)?
//!   3. Does `associated_creator_vault` receive the token transfer instead of
//!      `creator_vault` getting a raw lamport credit?
//!
//! Usage: cargo run --bin inspect_buy_v2_tx [-- <rpc-url>]
//!
//! Scans real `pump` program signatures (paginated, parallel-fetched),
//! filtering to `buy_v2` instructions whose `quote_mint` account (position 3,
//! 0-indexed 2) is NOT the wrapped-SOL mint, and fully decodes/prints the
//! first successful one found.

use solana_program::pubkey::Pubkey;
use solana_rpc_client::rpc_client::{GetConfirmedSignaturesForAddress2Config, RpcClient};
use solana_rpc_client_api::config::RpcTransactionConfig;
use solana_transaction_status_client_types::{EncodedTransaction, UiMessage, UiTransactionEncoding};
use std::str::FromStr;
use std::sync::{Arc, Mutex};
use std::time::Duration;

const PUMP_PROGRAM_ID: &str = "6EF8rrecthR5Dkzon8Nwu78hRvfCKubJ14M5uBEwF6P";
const WSOL_MINT: &str = "So11111111111111111111111111111111111111112";
const BUY_V2_DISCRIMINATOR: [u8; 8] = [184, 23, 238, 97, 103, 197, 211, 61];

const PAGE_SIZE: usize = 1000;
const MAX_PAGES: usize = 40;
const NUM_WORKERS: usize = 16;

fn pk(s: &str) -> Pubkey {
    Pubkey::from_str(s).unwrap()
}

/// Returns `Some(quote_mint_string)` if this signature is a successful
/// `buy_v2` on a non-WSOL quote mint.
fn check_signature(client: &RpcClient, sig_str: &str) -> Option<()> {
    let signature = solana_signature::Signature::from_str(sig_str).ok()?;
    let tx = client
        .get_transaction_with_config(
            &signature,
            RpcTransactionConfig { encoding: Some(UiTransactionEncoding::Json), commitment: None, max_supported_transaction_version: Some(0) },
        )
        .ok()?;
    let meta = tx.transaction.meta.as_ref()?;
    if meta.err.is_some() {
        return None;
    }
    let EncodedTransaction::Json(ui_tx) = &tx.transaction.transaction else { return None };
    let UiMessage::Raw(raw_msg) = &ui_tx.message else { return None };
    let mut all_keys: Vec<String> = raw_msg.account_keys.clone();
    if let solana_transaction_status_client_types::option_serializer::OptionSerializer::Some(loaded) = &meta.loaded_addresses {
        all_keys.extend(loaded.writable.iter().cloned());
        all_keys.extend(loaded.readonly.iter().cloned());
    }
    let pump_str = PUMP_PROGRAM_ID.to_string();

    // Top-level instructions actually invoking the real pump program.
    for ix in &raw_msg.instructions {
        let Ok(data) = bs58::decode(&ix.data).into_vec() else { continue };
        if data.len() < 8 || data[0..8] != BUY_V2_DISCRIMINATOR {
            continue;
        }
        let prog_key = all_keys.get(ix.program_id_index as usize)?;
        if prog_key != &pump_str {
            continue; // same discriminator, different (wrapper) program -- not a real call
        }
        let quote_mint_idx = ix.accounts.get(2).copied()? as usize;
        let quote_mint_key = all_keys.get(quote_mint_idx)?;
        if quote_mint_key != WSOL_MINT {
            return Some(());
        }
    }

    // An 8-byte Anchor discriminator is just a hash of the instruction NAME
    // string -- it isn't tied to a specific program, so a wrapper/router
    // program with its own handler literally named `buy_v2` produces the
    // identical bytes. Also check nested CPIs into the real pump program
    // specifically (by resolved program_id, not just matching bytes).
    if let solana_transaction_status_client_types::option_serializer::OptionSerializer::Some(inner_list) = &meta.inner_instructions {
        for group in inner_list {
            for inner_ix in &group.instructions {
                if let solana_transaction_status_client_types::UiInstruction::Compiled(compiled) = inner_ix {
                    let Ok(data) = bs58::decode(&compiled.data).into_vec() else { continue };
                    if data.len() < 8 || data[0..8] != BUY_V2_DISCRIMINATOR {
                        continue;
                    }
                    let prog_key = all_keys.get(compiled.program_id_index as usize)?;
                    if prog_key != &pump_str {
                        continue;
                    }
                    let quote_mint_idx = compiled.accounts.get(2).copied()? as usize;
                    let quote_mint_key = all_keys.get(quote_mint_idx)?;
                    if quote_mint_key != WSOL_MINT {
                        return Some(());
                    }
                }
            }
        }
    }
    None
}

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
                    panic!("re-fetch failed after {attempts} attempts: {e:?}");
                }
                println!("  (transient RPC error, retrying: {e:?})");
                std::thread::sleep(Duration::from_millis(1000));
            }
        }
    };
    let EncodedTransaction::Json(ui_tx) = &tx.transaction.transaction else { panic!("not JSON") };
    let UiMessage::Raw(raw_msg) = &ui_tx.message else { panic!("not raw message") };
    let mut all_keys: Vec<String> = raw_msg.account_keys.clone();
    let meta = tx.transaction.meta.as_ref().expect("meta missing");
    if let solana_transaction_status_client_types::option_serializer::OptionSerializer::Some(loaded) = &meta.loaded_addresses {
        all_keys.extend(loaded.writable.iter().cloned());
        all_keys.extend(loaded.readonly.iter().cloned());
    }

    const NAMES: &[&str] = &[
        "global", "base_mint", "quote_mint", "base_token_program", "quote_token_program",
        "associated_token_program", "fee_recipient", "associated_quote_fee_recipient",
        "buyback_fee_recipient", "associated_quote_buyback_fee_recipient", "bonding_curve",
        "associated_base_bonding_curve", "associated_quote_bonding_curve", "user",
        "associated_base_user", "associated_quote_user", "creator_vault", "associated_creator_vault",
        "sharing_config", "global_volume_accumulator", "user_volume_accumulator",
        "associated_user_volume_accumulator", "fee_config", "fee_program", "system_program",
        "event_authority", "program",
    ];
    let pump_str = PUMP_PROGRAM_ID.to_string();
    let print_decoded = |source: &str, prog_idx: usize, accounts: &[u8], data: &[u8]| {
        println!("\n=== Found real pump-program buy_v2 in tx {sig_str} ({source}) ===");
        println!("program_id at this call = {}", all_keys.get(prog_idx).cloned().unwrap_or_default());
        println!("Instruction data ({} bytes): {:02x?}", data.len(), data);
        println!("\nAccounts ({}), in documented order:", accounts.len());
        for (i, &account_index) in accounts.iter().enumerate() {
            let idx = account_index as usize;
            let key_str = all_keys.get(idx).cloned().unwrap_or_default();
            let name = NAMES.get(i).copied().unwrap_or("?");
            println!("  [{i}] {name} = {key_str}");
        }
    };

    let mut found = false;

    // Top-level first (a clean, unwrapped direct call, if one exists).
    for ix in &raw_msg.instructions {
        let Ok(data) = bs58::decode(&ix.data).into_vec() else { continue };
        if data.len() < 8 || data[0..8] != BUY_V2_DISCRIMINATOR {
            continue;
        }
        if all_keys.get(ix.program_id_index as usize) != Some(&pump_str) {
            continue; // same discriminator bytes, different (wrapper) program
        }
        print_decoded("top-level", ix.program_id_index as usize, &ix.accounts, &data);
        found = true;
        break;
    }

    // Otherwise, the real call is nested inside a wrapper/router program's
    // own CPI -- an Anchor discriminator is just a hash of the instruction
    // NAME string, so a wrapper with its own `buy_v2`-named handler produces
    // identical bytes; only the resolved `program_id` distinguishes them.
    if !found {
        if let solana_transaction_status_client_types::option_serializer::OptionSerializer::Some(inner_list) = &meta.inner_instructions {
            'outer: for group in inner_list {
                for inner_ix in &group.instructions {
                    if let solana_transaction_status_client_types::UiInstruction::Compiled(compiled) = inner_ix {
                        let Ok(data) = bs58::decode(&compiled.data).into_vec() else { continue };
                        if data.len() < 8 || data[0..8] != BUY_V2_DISCRIMINATOR {
                            continue;
                        }
                        if all_keys.get(compiled.program_id_index as usize) != Some(&pump_str) {
                            continue;
                        }
                        print_decoded("nested CPI", compiled.program_id_index as usize, &compiled.accounts, &data);
                        found = true;
                        break 'outer;
                    }
                }
            }
        }
    }

    if !found {
        println!("\nCould not locate a real pump-program buy_v2 call (top-level or nested) in tx {sig_str} -- the discriminator match was a false positive from an unrelated program.");
        return;
    }

    println!("\nmeta.err = {:?}", meta.err);
    println!("\nFull meta.inner_instructions (raw debug dump):");
    println!("{:#?}", meta.inner_instructions);
    if let solana_transaction_status_client_types::option_serializer::OptionSerializer::Some(logs) = &meta.log_messages {
        println!("\nlog_messages:");
        for l in logs {
            println!("  {l}");
        }
    }
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
                        if check_signature(&client, sig_str).is_some() {
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

    println!("\nNo non-WSOL buy_v2 instruction found across {scanned} scanned signature(s).");
}
