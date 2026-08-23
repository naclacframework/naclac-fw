//! `inspect_buy_tx` revealed the assumed account ORDER for `buy` is wrong,
//! not just the buyback slot — the real mint (identifiable by its "pump"
//! vanity suffix) sits at ix-relative position [9], not [3] like every
//! prior probe (probe15/16/17) assumed. Rather than keep guessing positions,
//! this identifies EVERY account in a real `buy` by its actual on-chain
//! owner/data shape/discriminator (Mint vs TokenAccount vs each of our
//! already-confirmed real discriminators), so the account list can be
//! rebuilt from evidence instead of assumption.
//!
//! Usage: cargo run --bin identify_buy_accounts [-- <rpc-url>]

use solana_program::pubkey::Pubkey;
use solana_rpc_client::rpc_client::RpcClient;
use solana_rpc_client_api::config::RpcTransactionConfig;
use solana_transaction_status_client_types::{EncodedTransaction, UiMessage, UiTransactionEncoding};
use std::str::FromStr;
use std::time::Duration;

const PUMP_PROGRAM_ID: &str = "6EF8rrecthR5Dkzon8Nwu78hRvfCKubJ14M5uBEwF6P";
const PUMP_FEES_PROGRAM_ID: &str = "pfeeUxB6jkeY1Hxd7CsFCAjcbHA9rWtchMGdZ6VojVZ";
const TOKEN_PROGRAM_ID: &str = "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA";
const TOKEN_2022_PROGRAM_ID: &str = "TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb";
const SYSTEM_PROGRAM_ID: &str = "11111111111111111111111111111111";
const MAYHEM_PROGRAM_ID: &str = "MAyhSmzXzV1pTf7LsNkrNwkWKTo4ougAJ1PPg47MD4e";
const BUY_DISCRIMINATOR: [u8; 8] = [102, 6, 61, 18, 1, 218, 235, 234];

const GLOBAL_DISCRIMINATOR: [u8; 8] = [167, 232, 232, 177, 200, 108, 114, 127];
const BONDING_CURVE_DISCRIMINATOR: [u8; 8] = [23, 183, 248, 55, 96, 216, 172, 96];
const FEE_CONFIG_DISCRIMINATOR: [u8; 8] = [143, 52, 146, 187, 219, 123, 76, 155];
const GLOBAL_VOLUME_ACCUMULATOR_DISCRIMINATOR: [u8; 8] = [202, 42, 246, 43, 142, 190, 30, 255];
const USER_VOLUME_ACCUMULATOR_DISCRIMINATOR: [u8; 8] = [86, 255, 112, 14, 102, 53, 154, 250];
const BUYBACK_VAULT_DISCRIMINATOR: [u8; 8] = [153, 166, 71, 144, 179, 189, 137, 251];

fn pk(s: &str) -> Pubkey {
    Pubkey::from_str(s).unwrap()
}

fn identify(client: &RpcClient, key_str: &str, buyback_recipients: &[Pubkey]) -> String {
    let Ok(key) = Pubkey::from_str(key_str) else { return "(invalid key)".to_string() };

    if key == pk(SYSTEM_PROGRAM_ID) {
        return "= System Program".to_string();
    }
    if key == pk(TOKEN_PROGRAM_ID) {
        return "= Token Program".to_string();
    }
    if key == pk(TOKEN_2022_PROGRAM_ID) {
        return "= Token-2022 Program".to_string();
    }
    if key == pk(PUMP_PROGRAM_ID) {
        return "= pump program itself (self-reference)".to_string();
    }
    if key == pk(PUMP_FEES_PROGRAM_ID) {
        return "= pump_fees program itself".to_string();
    }
    if key == pk(MAYHEM_PROGRAM_ID) {
        return "= Mayhem program".to_string();
    }
    if key_str.ends_with("pump") {
        return "*** VANITY SUFFIX 'pump' -> almost certainly the MINT ***".to_string();
    }
    if let Some(idx) = buyback_recipients.iter().position(|&r| r == key) {
        return format!("MATCHES global.buyback_fee_recipients[{idx}]");
    }

    match client.get_account(&key) {
        Ok(acc) => {
            let disc: Option<[u8; 8]> = acc.data.get(0..8).and_then(|s| s.try_into().ok());
            let known = match disc {
                Some(d) if d == GLOBAL_DISCRIMINATOR => " <- Global discriminator",
                Some(d) if d == BONDING_CURVE_DISCRIMINATOR => " <- BondingCurve discriminator",
                Some(d) if d == FEE_CONFIG_DISCRIMINATOR => " <- FeeConfig discriminator",
                Some(d) if d == GLOBAL_VOLUME_ACCUMULATOR_DISCRIMINATOR => " <- GlobalVolumeAccumulator discriminator",
                Some(d) if d == USER_VOLUME_ACCUMULATOR_DISCRIMINATOR => " <- UserVolumeAccumulator discriminator",
                Some(d) if d == BUYBACK_VAULT_DISCRIMINATOR => " <- BuybackVault discriminator",
                _ => "",
            };
            let shape = if acc.owner == pk(TOKEN_PROGRAM_ID) || acc.owner == pk(TOKEN_2022_PROGRAM_ID) {
                if acc.data.len() == 82 {
                    " <- SPL Mint (82 bytes)"
                } else if acc.data.len() == 165 {
                    " <- SPL TokenAccount (165 bytes)"
                } else {
                    ""
                }
            } else {
                ""
            };
            format!("owner={} data_len={} lamports={}{known}{shape}", acc.owner, acc.data.len(), acc.lamports)
        }
        Err(_) => "(account fetch failed - likely not yet created / ALT resolution issue)".to_string(),
    }
}

fn main() {
    let rpc_url = std::env::args().nth(1).unwrap_or_else(|| "https://api.mainnet-beta.solana.com".to_string());
    let client = RpcClient::new_with_timeout(rpc_url, Duration::from_secs(60));
    let pump_program = pk(PUMP_PROGRAM_ID);

    let (global, _) = Pubkey::find_program_address(&[b"global"], &pump_program);
    let global_account = client.get_account(&global).expect("failed to fetch global");
    let start = 8 + 733;
    let mut buyback_recipients = Vec::new();
    for i in 0..8 {
        let off = start + i * 32;
        buyback_recipients.push(Pubkey::try_from(&global_account.data[off..off + 32]).unwrap());
    }
    println!("global = {global}");
    println!("global.buyback_fee_recipients:");
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
                println!("Total accounts in this ix: {}\n", ix.accounts.len());

                for (i, &account_index) in ix.accounts.iter().enumerate() {
                    let idx = account_index as usize;
                    let key_str = all_keys.get(idx).cloned().unwrap_or_default();
                    let info = identify(&client, &key_str, &buyback_recipients);
                    println!("  [{i}] key={key_str} | {info}");
                }
                return;
            }
        }
    }

    println!("No `buy` instruction found in the most recent signatures for this program.");
}
