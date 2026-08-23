//! `find_buy_tx` happened to land on a mayhem-mode coin (a whole separate,
//! undocumented feature system), which explained the 6 extra unidentified
//! accounts beyond the buyback recipient — not a property of `buy` in
//! general. This scans for a `buy` transaction whose bonding_curve is
//! `is_mayhem_mode = 0 AND is_cashback_coin = 0` (a plain trade), so the
//! account list should need only the documented 16 + 1 buyback recipient.
//!
//! Usage: cargo run --bin find_plain_buy_tx [-- <rpc-url>]

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

    let sigs = client
        .get_signatures_for_address(&pump_program)
        .expect("get_signatures_for_address failed");

    println!("Scanning {} recent signature(s) for a plain (non-mayhem, non-cashback) `buy`...", sigs.len());

    let mut checked = 0;
    for sig_info in &sigs {
        if checked >= 60 {
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
                checked += 1;
                let total_accounts = ix.accounts.len();

                // mint is at instruction-relative position [3] per cross_reference_buy_tx's confirmed layout.
                let Some(&mint_idx) = ix.accounts.get(3) else { continue };
                let Some(mint_str) = all_keys.get(mint_idx as usize) else { continue };
                let Ok(mint) = Pubkey::from_str(mint_str) else { continue };

                let (bonding_curve, _) = Pubkey::find_program_address(&[b"bonding-curve", mint.as_ref()], &pump_program);
                let Ok(bc_account) = client.get_account(&bonding_curve) else { continue };
                if bc_account.data.len() < 8 + 40 + 1 + 32 + 2 {
                    continue;
                }
                let is_mayhem_mode = bc_account.data[8 + 40 + 1 + 32];
                let is_cashback_coin = bc_account.data[8 + 40 + 1 + 32 + 1];

                println!(
                    "sig={} mint={mint_str} total_accounts={total_accounts} is_mayhem_mode={is_mayhem_mode} is_cashback_coin={is_cashback_coin}",
                    sig_info.signature
                );

                if is_mayhem_mode == 0 && is_cashback_coin == 0 {
                    println!("  ^ PLAIN TRADE FOUND — total_accounts={total_accounts} (expect 17 if only +buyback_recipient)");
                }
            }
        }
    }

    println!("\nChecked {checked} `buy` instruction(s).");
}
