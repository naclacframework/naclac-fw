//! Finds and decodes a real, successful `migrate_v2` transaction to settle
//! `probe45.rs`'s open question with ground truth instead of more guessing:
//! what exactly does `migrate_v2` need in `remaining_accounts`? Two guesses
//! (`[creator, creator_quote_ata]` and `[creator, creator_quote_ata,
//! boost_vault_authority, boost_vault]`) gave different, inconclusive
//! results against real bytecode in litesvm — this reads what a real,
//! already-executed migration actually passed.
//!
//! Same false-positive-discriminator guard as `inspect_buy_v2_tx.rs`: an
//! Anchor discriminator is just a hash of the instruction name string, not
//! tied to a specific program, so this verifies `program_id` too, for both
//! top-level and nested CPI calls.
//!
//! Usage: cargo run --bin inspect_migrate_v2_tx [-- <rpc-url>]

use solana_program::pubkey::Pubkey;
use solana_rpc_client::rpc_client::{GetConfirmedSignaturesForAddress2Config, RpcClient};
use solana_rpc_client_api::config::RpcTransactionConfig;
use solana_transaction_status_client_types::{EncodedTransaction, UiInstruction, UiMessage, UiTransactionEncoding};
use std::str::FromStr;
use std::sync::{Arc, Mutex};
use std::time::Duration;

const PUMP_PROGRAM_ID: &str = "6EF8rrecthR5Dkzon8Nwu78hRvfCKubJ14M5uBEwF6P";
const MIGRATE_V2_DISCRIMINATOR: [u8; 8] = [187, 203, 18, 31, 206, 237, 254, 41];

const PAGE_SIZE: usize = 1000;
const MAX_PAGES: usize = 60;
const NUM_WORKERS: usize = 16;

fn pk(s: &str) -> Pubkey {
    Pubkey::from_str(s).unwrap()
}

fn tx_has_real_migrate_v2(client: &RpcClient, sig_str: &str) -> bool {
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
    let mut all_keys: Vec<String> = raw_msg.account_keys.clone();
    if let solana_transaction_status_client_types::option_serializer::OptionSerializer::Some(loaded) = &meta.loaded_addresses {
        all_keys.extend(loaded.writable.iter().cloned());
        all_keys.extend(loaded.readonly.iter().cloned());
    }
    let pump_str = PUMP_PROGRAM_ID.to_string();

    for ix in &raw_msg.instructions {
        let Ok(data) = bs58::decode(&ix.data).into_vec() else { continue };
        if data.len() >= 8 && data[0..8] == MIGRATE_V2_DISCRIMINATOR && all_keys.get(ix.program_id_index as usize) == Some(&pump_str) {
            return true;
        }
    }
    if let solana_transaction_status_client_types::option_serializer::OptionSerializer::Some(inner_list) = &meta.inner_instructions {
        for group in inner_list {
            for inner_ix in &group.instructions {
                if let UiInstruction::Compiled(compiled) = inner_ix {
                    let Ok(data) = bs58::decode(&compiled.data).into_vec() else { continue };
                    if data.len() >= 8 && data[0..8] == MIGRATE_V2_DISCRIMINATOR && all_keys.get(compiled.program_id_index as usize) == Some(&pump_str) {
                        return true;
                    }
                }
            }
        }
    }
    false
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
    let pump_str = PUMP_PROGRAM_ID.to_string();

    const NAMES: &[&str] = &[
        "global", "withdraw_authority", "base_mint", "quote_mint", "bonding_curve",
        "associated_base_bonding_curve", "associated_quote_bonding_curve", "user", "system_program",
        "pump_amm", "pool", "pool_authority", "pool_authority_mint_account", "pool_authority_quote_account",
        "amm_global_config", "lp_mint", "user_pool_token_account", "pool_base_token_account",
        "pool_quote_token_account", "base_token_program", "quote_token_program", "token_2022_program",
        "associated_token_program", "pump_amm_event_authority", "rent", "event_authority", "program",
    ];

    let print_decoded = |source: &str, accounts: &[u8], data: &[u8]| {
        println!("\n=== Found real pump-program migrate_v2 in tx {sig_str} ({source}) ===");
        println!("Instruction data ({} bytes): {:02x?}", data.len(), data);
        println!("\nAccounts ({}):", accounts.len());
        for (i, &account_index) in accounts.iter().enumerate() {
            let idx = account_index as usize;
            let key_str = all_keys.get(idx).cloned().unwrap_or_default();
            let name = NAMES.get(i).copied().unwrap_or("REMAINING_ACCOUNT");
            println!("  [{i}] {name} = {key_str}");
        }
    };

    let mut found = false;
    for ix in &raw_msg.instructions {
        let Ok(data) = bs58::decode(&ix.data).into_vec() else { continue };
        if data.len() < 8 || data[0..8] != MIGRATE_V2_DISCRIMINATOR || all_keys.get(ix.program_id_index as usize) != Some(&pump_str) {
            continue;
        }
        print_decoded("top-level", &ix.accounts, &data);
        found = true;
        break;
    }
    if !found {
        if let solana_transaction_status_client_types::option_serializer::OptionSerializer::Some(inner_list) = &meta.inner_instructions {
            'outer: for group in inner_list {
                for inner_ix in &group.instructions {
                    if let UiInstruction::Compiled(compiled) = inner_ix {
                        let Ok(data) = bs58::decode(&compiled.data).into_vec() else { continue };
                        if data.len() < 8 || data[0..8] != MIGRATE_V2_DISCRIMINATOR || all_keys.get(compiled.program_id_index as usize) != Some(&pump_str) {
                            continue;
                        }
                        print_decoded("nested CPI", &compiled.accounts, &data);
                        found = true;
                        break 'outer;
                    }
                }
            }
        }
    }
    if !found {
        println!("\nCould not locate a real migrate_v2 call in tx {sig_str}.");
        return;
    }

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
                        if tx_has_real_migrate_v2(&client, sig_str) {
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

    println!("\nNo real migrate_v2 instruction found across {scanned} scanned signature(s).");
}
