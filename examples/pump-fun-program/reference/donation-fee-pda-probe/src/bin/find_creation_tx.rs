//! `find_pool` proved there's NO Pool account at all for a real, ungraduated
//! `DonationFeePda`'s base_mint — so `pool`'s real content isn't mint-relevant
//! at all, and guessing its derivation is the wrong approach entirely. This
//! instead pulls the ACTUAL transaction that created a given real
//! `DonationFeePda` account and prints its exact account list, in order —
//! ground truth for what `pool` (and everything else) really was, no
//! reverse-engineering required.
//!
//! Usage: cargo run --bin find_creation_tx -- <donation_fee_pda_address> [rpc-url]

use solana_program::pubkey::Pubkey;
use solana_rpc_client::rpc_client::RpcClient;
use solana_rpc_client_api::config::RpcTransactionConfig;
use solana_transaction_status_client_types::{
    EncodedTransaction, UiMessage, UiTransactionEncoding,
};
use std::str::FromStr;
use std::time::Duration;

const CREATE_DONATION_FEE_PDA_DISCRIMINATOR: [u8; 8] = [244, 139, 16, 88, 14, 255, 122, 26];

fn main() {
    let address_str = std::env::args()
        .nth(1)
        .expect("usage: find_creation_tx <donation_fee_pda_address> [rpc-url]");
    let address = Pubkey::from_str(&address_str).expect("invalid pubkey");
    let rpc_url = std::env::args()
        .nth(2)
        .unwrap_or_else(|| "https://api.mainnet-beta.solana.com".to_string());
    let client = RpcClient::new_with_timeout(rpc_url, Duration::from_secs(60));

    // Newest-first (RPC default order) — reversed below to check oldest first,
    // since the creation tx is the earliest signature touching this account.
    // Capped at 1000 (one page); good enough for a young/low-traffic account.
    let all_sigs = client
        .get_signatures_for_address(&address)
        .expect("get_signatures_for_address failed");

    println!("Found {} total signature(s) for {address}. Checking oldest first for CreateDonationFeePda...", all_sigs.len());

    for sig_info in all_sigs.iter().rev() {
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

        for ix in &raw_msg.instructions {
            let Ok(data) = bs58::decode(&ix.data).into_vec() else { continue };
            if data.len() >= 8 && data[0..8] == CREATE_DONATION_FEE_PDA_DISCRIMINATOR {
                println!("\n=== Found CreateDonationFeePda in tx {} ===", sig_info.signature);
                println!("Account keys used by this instruction (in order):");
                for (i, &account_index) in ix.accounts.iter().enumerate() {
                    let key = raw_msg.account_keys.get(account_index as usize);
                    println!("  [{i}] index={account_index} key={:?}", key);
                }
                return;
            }
        }
    }

    println!("No CreateDonationFeePda instruction found in this account's transaction history.");
}
