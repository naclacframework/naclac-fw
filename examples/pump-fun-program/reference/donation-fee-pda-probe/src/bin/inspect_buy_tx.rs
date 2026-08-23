//! `probe16` (litesvm) fails with `BuybackFeeRecipientMissing` even after
//! setting all 8 `global.buyback_fee_recipients` entries to the same pubkey
//! and passing that same pubkey as the sole remaining account — despite
//! `sample_buyback_rotation.rs` previously confirming a real transaction's
//! remaining-account address IS a member of the real, live
//! `global.buyback_fee_recipients` array (offset 8+733, already verified).
//! `pump.so` itself is confirmed NOT stale (fresh dump hash matches).
//!
//! This fetches a fresh real `buy` transaction and inspects EVERY account
//! beyond the 16 documented ones: is it really just one remaining account,
//! is it writable, and what does its real on-chain owner/data shape look
//! like (plain wallet vs PDA vs token account) — to find what we're
//! actually missing rather than continue guessing locally.
//!
//! Usage: cargo run --bin inspect_buy_tx [-- <rpc-url>]

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
    let rpc_url = std::env::args().nth(1).unwrap_or_else(|| "https://api.mainnet-beta.solana.com".to_string());
    let client = RpcClient::new_with_timeout(rpc_url, Duration::from_secs(60));
    let pump_program = pk(PUMP_PROGRAM_ID);

    // Real, live `global.buyback_fee_recipients[8]` — already-confirmed offset.
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

    let sigs = client.get_signatures_for_address(&pump_program).expect("get_signatures_for_address failed");
    println!("\nScanning {} recent signature(s) for a `buy` instruction...", sigs.len());

    for sig_info in &sigs {
        let Ok(signature) = solana_signature::Signature::from_str(&sig_info.signature) else { continue };
        let tx = match client.get_transaction_with_config(
            &signature,
            RpcTransactionConfig { encoding: Some(UiTransactionEncoding::Json), commitment: None, max_supported_transaction_version: Some(0) },
        ) {
            Ok(tx) => tx,
            Err(_) => continue,
        };

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
                println!("\n=== Found `buy` in tx {} ===", sig_info.signature);
                println!("Instruction data ({} bytes): {:02x?}", data.len(), data);
                println!("Total accounts in this ix: {}\n", ix.accounts.len());

                for (i, &account_index) in ix.accounts.iter().enumerate() {
                    let idx = account_index as usize;
                    let key_str = all_keys.get(idx).cloned().unwrap_or_default();
                    let mut extra = String::new();
                    if i >= 16 {
                        if let Ok(key) = Pubkey::from_str(&key_str) {
                            let matched = buyback_recipients.iter().position(|&r| r == key);
                            if let Ok(acc) = client.get_account(&key) {
                                extra = format!(
                                    " | owner={} data_len={} lamports={} buyback_match={:?}",
                                    acc.owner, acc.data.len(), acc.lamports, matched
                                );
                            } else {
                                extra = format!(" | (account fetch failed) buyback_match={:?}", matched);
                            }
                        }
                    }
                    println!("  [{i}] index={idx} key={key_str}{extra}");
                }
                return;
            }
        }
    }

    println!("No `buy` instruction found in the most recent signatures for this program.");
}
