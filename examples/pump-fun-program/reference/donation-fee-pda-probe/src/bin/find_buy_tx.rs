//! Blind probe-and-guess iteration on `buy`'s account list (specifically the
//! "BuybackFeeRecipientMissing" remaining-account requirement) stalled after
//! several attempts. Ground truth instead: fetch a real, recent mainnet
//! transaction that called `buy` and print its exact account list in order —
//! no guessing about which remaining accounts exist or what they contain.
//!
//! Usage: cargo run --bin find_buy_tx [-- <rpc-url>]

use solana_program::pubkey::Pubkey;
use solana_rpc_client::rpc_client::RpcClient;
use solana_rpc_client_api::config::RpcTransactionConfig;
use solana_transaction_status_client_types::{EncodedTransaction, UiMessage, UiTransactionEncoding};
use std::str::FromStr;
use std::time::Duration;

const PUMP_PROGRAM_ID: &str = "6EF8rrecthR5Dkzon8Nwu78hRvfCKubJ14M5uBEwF6P";
const BUY_DISCRIMINATOR: [u8; 8] = [102, 6, 61, 18, 1, 218, 235, 234];

fn main() {
    let rpc_url = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "https://api.mainnet-beta.solana.com".to_string());
    let client = RpcClient::new_with_timeout(rpc_url, Duration::from_secs(60));

    let pump_program = Pubkey::from_str(PUMP_PROGRAM_ID).unwrap();

    let sigs = client
        .get_signatures_for_address(&pump_program)
        .expect("get_signatures_for_address failed");

    println!("Scanning {} recent signature(s) for a `buy` instruction...", sigs.len());

    for sig_info in &sigs {
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

        let EncodedTransaction::Json(ui_tx) = &tx.transaction.transaction else { continue };
        let UiMessage::Raw(raw_msg) = &ui_tx.message else { continue };

        // Static account_keys, then ALT-loaded writable, then ALT-loaded readonly —
        // matches Solana's own index convention for versioned transactions.
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
                println!("Account keys used by this instruction (in order):");
                for (i, &account_index) in ix.accounts.iter().enumerate() {
                    let key = all_keys.get(account_index as usize);
                    println!("  [{i}] index={account_index} key={:?}", key);
                }
                println!("\nTotal accounts in this ix: {}", ix.accounts.len());
                return;
            }
        }
    }

    println!("No `buy` instruction found in the most recent signatures for this program — try again (pump is high-traffic, this should be rare) or pass a private RPC URL.");
}
