//! `cross_reference_buy_tx` confirmed one real `buy`'s selected
//! `buyback_fee_recipients` entry was array index 2, not 0 — so there's a
//! selection/rotation algorithm, not "always the first entry". This
//! collects multiple real `buy` transactions (different mints/slots) and
//! prints each one's selected index alongside its slot and mint, to find
//! the pattern (round-robin counter? hash of mint? hash of slot?) rather
//! than guess from one data point.
//!
//! Usage: cargo run --bin sample_buyback_rotation [-- <rpc-url>]

use solana_program::pubkey::Pubkey;
use solana_rpc_client::rpc_client::RpcClient;
use solana_rpc_client_api::config::RpcTransactionConfig;
use solana_transaction_status_client_types::{EncodedTransaction, UiMessage, UiTransactionEncoding};
use std::str::FromStr;
use std::time::Duration;

const PUMP_PROGRAM_ID: &str = "6EF8rrecthR5Dkzon8Nwu78hRvfCKubJ14M5uBEwF6P";
const BUY_DISCRIMINATOR: [u8; 8] = [102, 6, 61, 18, 1, 218, 235, 234];

fn pk(s: &str) -> Pubkey {
    Pubkey::from_str(s).unwrap()
}

fn main() {
    let rpc_url = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "https://api.mainnet-beta.solana.com".to_string());
    let client = RpcClient::new_with_timeout(rpc_url, Duration::from_secs(60));

    let pump_program = pk(PUMP_PROGRAM_ID);

    // Fetch global's current buyback_fee_recipients[8] once.
    let (global, _) = Pubkey::find_program_address(&[b"global"], &pump_program);
    let global_account = client.get_account(&global).expect("failed to fetch global");
    let start = 8 + 733;
    let mut buyback_recipients = Vec::new();
    for i in 0..8 {
        let off = start + i * 32;
        buyback_recipients.push(Pubkey::try_from(&global_account.data[off..off + 32]).unwrap());
    }
    println!("Current global.buyback_fee_recipients:");
    for (i, r) in buyback_recipients.iter().enumerate() {
        println!("  [{i}] {r}");
    }

    let sigs = client
        .get_signatures_for_address(&pump_program)
        .expect("get_signatures_for_address failed");

    println!("\nScanning {} recent signature(s) for `buy` instructions...\n", sigs.len());

    let mut samples_found = 0;
    for sig_info in &sigs {
        if samples_found >= 15 {
            break;
        }
        let Ok(signature) = solana_signature::Signature::from_str(&sig_info.signature) else { continue };
        let tx = match client.get_transaction_with_config(
            &signature,
            RpcTransactionConfig {
                encoding: Some(UiTransactionEncoding::Json),
                commitment: None,
                max_supported_transaction_version: Some(0),
            },
        ) {
            Ok(tx) => tx,
            Err(_) => continue,
        };

        let slot = tx.slot;
        let EncodedTransaction::Json(ui_tx) = &tx.transaction.transaction else { continue };
        let UiMessage::Raw(raw_msg) = &ui_tx.message else { continue };

        let mut all_keys: Vec<String> = raw_msg.account_keys.clone();
        if let Some(meta) = &tx.transaction.meta {
            if let solana_transaction_status_client_types::option_serializer::OptionSerializer::Some(loaded) = &meta.loaded_addresses {
                all_keys.extend(loaded.writable.iter().cloned());
                all_keys.extend(loaded.readonly.iter().cloned());
            }
        }

        for ix in &raw_msg.instructions {
            let Ok(data) = bs58::decode(&ix.data).into_vec() else { continue };
            if data.len() >= 8 && data[0..8] == BUY_DISCRIMINATOR {
                // mint is at instruction-relative position [3] per cross_reference_buy_tx's confirmed layout.
                let Some(&mint_idx) = ix.accounts.get(3) else { continue };
                let Some(mint_str) = all_keys.get(mint_idx as usize) else { continue };

                // Find which (if any) account in this ix matches a buyback_fee_recipients entry.
                let mut found_index = None;
                for &acc_idx in &ix.accounts {
                    if let Some(key_str) = all_keys.get(acc_idx as usize) {
                        if let Ok(key) = Pubkey::from_str(key_str) {
                            if let Some(idx) = buyback_recipients.iter().position(|&r| r == key) {
                                found_index = Some(idx);
                                break;
                            }
                        }
                    }
                }

                match found_index {
                    Some(idx) => {
                        println!("slot={slot} mint={mint_str} buyback_index={idx} slot%8={} sig={}", slot % 8, sig_info.signature);
                        samples_found += 1;
                    }
                    None => {
                        println!("slot={slot} mint={mint_str} buyback_index=NONE-FOUND sig={}", sig_info.signature);
                    }
                }
            }
        }
    }

    println!("\nCollected {samples_found} sample(s) with a resolved buyback index.");
}
