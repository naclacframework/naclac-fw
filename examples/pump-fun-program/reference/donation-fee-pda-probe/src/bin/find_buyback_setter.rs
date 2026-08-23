//! Every real instruction name in `pump.json` has been checked and ruled
//! out as the setter for `Global.buyback_fee_recipients` (`set_params`
//! confirmed unchanged even with 16 remaining accounts; `update_buyback_config`
//! only touches `buyback_basis_points`; `pump_fees::initialize_buyback` has
//! no CPI path to `pump`'s `Global`; `initialize` confirmed leaves it
//! zeroed). This scans `Global`'s own real transaction history directly,
//! filtered to only instructions where `global` was actually WRITABLE
//! (real `buy`/`sell` only ever read it, so this excludes the vast bulk of
//! routine trade traffic and should leave mostly genuine admin calls).
//! Fetches transaction bodies in parallel (sequential fetching of
//! thousands of transactions against public RPC is impractically slow —
//! confirmed earlier this session with `find_plain_buy_tx.rs`).
//!
//! Usage: cargo run --bin find_buyback_setter [-- <rpc-url>]

use solana_program::pubkey::Pubkey;
use solana_rpc_client::rpc_client::{GetConfirmedSignaturesForAddress2Config, RpcClient};
use solana_rpc_client_api::config::RpcTransactionConfig;
use solana_transaction_status_client_types::{EncodedTransaction, UiMessage, UiTransactionEncoding};
use std::collections::HashMap;
use std::str::FromStr;
use std::sync::{Arc, Mutex};
use std::time::Duration;

const PUMP_PROGRAM_ID: &str = "6EF8rrecthR5Dkzon8Nwu78hRvfCKubJ14M5uBEwF6P";
const MAX_PAGES: usize = 4; // 4 * 1000 = up to 4,000 signatures scanned
const NUM_WORKERS: usize = 16;

const KNOWN: &[(&str, [u8; 8])] = &[
    ("buy", [102, 6, 61, 18, 1, 218, 235, 234]),
    ("buy_exact_quote_in_v2", [194, 171, 28, 70, 104, 77, 91, 47]),
    ("buy_v2", [184, 23, 238, 97, 103, 197, 211, 61]),
    ("sell", [51, 230, 133, 164, 1, 127, 131, 173]),
    ("sell_v2", [93, 246, 130, 60, 231, 233, 64, 178]),
    ("set_params", [27, 234, 178, 52, 147, 2, 187, 141]),
    ("update_buyback_config", [251, 224, 171, 146, 160, 26, 113, 233]),
    ("update_global_authority", [227, 181, 74, 196, 208, 21, 97, 213]),
    ("set_reserved_fee_recipients", [111, 172, 162, 232, 114, 89, 213, 142]),
    ("set_virtual_quote_reserves", [101, 135, 191, 104, 9, 88, 20, 96]),
    ("toggle_cashback_enabled", [115, 103, 224, 255, 189, 89, 86, 195]),
    ("toggle_create_v2", [28, 255, 230, 240, 172, 107, 203, 171]),
    ("toggle_mayhem_mode", [1, 9, 111, 208, 100, 31, 255, 163]),
    ("initialize", [175, 175, 109, 31, 13, 152, 155, 237]),
    ("add_quote_mint", [111, 121, 21, 56, 40, 24, 94, 209]),
    ("remove_quote_mint", [177, 65, 223, 38, 88, 209, 158, 155]),
    ("migrate", [155, 234, 231, 146, 236, 158, 162, 30]),
];

fn pk(s: &str) -> Pubkey {
    Pubkey::from_str(s).unwrap()
}

fn process_signature(client: &RpcClient, sig_str: &str, global: &Pubkey, global_str: &str) -> Option<[u8; 8]> {
    let signature = solana_signature::Signature::from_str(sig_str).ok()?;
    let tx = client
        .get_transaction_with_config(
            &signature,
            RpcTransactionConfig { encoding: Some(UiTransactionEncoding::Json), commitment: None, max_supported_transaction_version: Some(0) },
        )
        .ok()?;

    let EncodedTransaction::Json(ui_tx) = &tx.transaction.transaction else { return None };
    let UiMessage::Raw(raw_msg) = &ui_tx.message else { return None };

    let mut all_keys: Vec<String> = raw_msg.account_keys.clone();
    let mut loaded_writable_count = 0usize;
    if let Some(meta) = &tx.transaction.meta {
        if let solana_transaction_status_client_types::option_serializer::OptionSerializer::Some(loaded) = &meta.loaded_addresses {
            loaded_writable_count = loaded.writable.len();
            all_keys.extend(loaded.writable.iter().cloned());
            all_keys.extend(loaded.readonly.iter().cloned());
        }
    }
    let static_len = raw_msg.account_keys.len();
    let num_readonly_unsigned = raw_msg.header.num_readonly_unsigned_accounts as usize;
    let num_readonly_signed = raw_msg.header.num_readonly_signed_accounts as usize;
    let num_signed = raw_msg.header.num_required_signatures as usize;
    let is_writable = |idx: usize| -> bool {
        if idx < static_len {
            if idx < num_signed { idx < num_signed - num_readonly_signed } else { idx < static_len - num_readonly_unsigned }
        } else {
            idx < static_len + loaded_writable_count
        }
    };

    let global_idx = all_keys.iter().position(|k| k == global_str)?;
    if !is_writable(global_idx) {
        return None;
    }
    let _ = global;

    for ix in &raw_msg.instructions {
        let Ok(data) = bs58::decode(&ix.data).into_vec() else { continue };
        if data.len() < 8 {
            continue;
        }
        if !ix.accounts.iter().any(|&a| a as usize == global_idx) {
            continue;
        }
        return Some(data[0..8].try_into().unwrap());
    }
    None
}

fn main() {
    let rpc_url = std::env::args().nth(1).unwrap_or_else(|| "https://api.mainnet-beta.solana.com".to_string());
    let pump_program = pk(PUMP_PROGRAM_ID);
    let (global, _) = Pubkey::find_program_address(&[b"global"], &pump_program);
    let global_str = global.to_string();
    println!("global = {global}\n");

    let known_map: HashMap<[u8; 8], &str> = KNOWN.iter().map(|(name, disc)| (*disc, *name)).collect();
    let disc_counts: Arc<Mutex<HashMap<[u8; 8], usize>>> = Arc::new(Mutex::new(HashMap::new()));

    let listing_client = RpcClient::new_with_timeout(rpc_url.clone(), Duration::from_secs(60));
    let mut before: Option<String> = None;
    let mut total_scanned = 0usize;

    for page in 0..MAX_PAGES {
        let config = GetConfirmedSignaturesForAddress2Config {
            before: before.as_deref().and_then(|s| solana_signature::Signature::from_str(s).ok()),
            until: None,
            limit: Some(1000),
            commitment: None,
        };
        let sigs = match listing_client.get_signatures_for_address_with_config(&global, config) {
            Ok(s) => s,
            Err(e) => {
                println!("RPC error on page {page}: {e:?} -- stopping.");
                break;
            }
        };
        if sigs.is_empty() {
            println!("No more signatures -- reached the start of Global's history at page {page}.");
            break;
        }
        before = Some(sigs.last().unwrap().signature.clone());
        total_scanned += sigs.len();
        println!("--- page {page}: {} signatures (total so far: {total_scanned}) -- fetching with {NUM_WORKERS} parallel workers ---", sigs.len());

        let sig_strings: Vec<String> = sigs.iter().map(|s| s.signature.clone()).collect();
        let chunks: Vec<Vec<String>> = sig_strings.chunks(sig_strings.len().div_ceil(NUM_WORKERS).max(1)).map(|c| c.to_vec()).collect();

        std::thread::scope(|scope| {
            for chunk in chunks {
                let rpc_url = rpc_url.clone();
                let global_str = global_str.clone();
                let disc_counts = Arc::clone(&disc_counts);
                let known_map = &known_map;
                scope.spawn(move || {
                    let client = RpcClient::new_with_timeout(rpc_url, Duration::from_secs(60));
                    for sig_str in &chunk {
                        if let Some(disc) = process_signature(&client, sig_str, &global, &global_str) {
                            let mut counts = disc_counts.lock().unwrap();
                            *counts.entry(disc).or_insert(0) += 1;
                            if !known_map.contains_key(&disc) {
                                println!("  >>> UNKNOWN disc={disc:02x?} in sig={sig_str} (global WRITABLE) <<<");
                            }
                        }
                    }
                });
            }
        });

        println!("  running totals:");
        for (disc, count) in disc_counts.lock().unwrap().iter() {
            let label = known_map.get(disc).copied().unwrap_or("UNKNOWN");
            println!("    {label:30} count={count}");
        }
    }

    println!("\nDone. Total signatures scanned: {total_scanned}.");
}
