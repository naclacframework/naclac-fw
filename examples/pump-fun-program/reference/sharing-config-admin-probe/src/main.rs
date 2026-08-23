//! One-shot empirical probe: does a real, un-reset `SharingConfig` account's
//! `admin` field equal `shareholders[0]` (the creator recorded at creation,
//! per docs/plan/fees-02-accounts-and-state.md)? No source material states
//! what `admin` is set to on creation directly — this checks real mainnet
//! accounts instead of guessing.
//!
//! Fetches every account owned by the real pump_fees program, keeps the ones
//! whose first 8 bytes match SharingConfig's discriminator, decodes each
//! directly from raw bytes per the real Anchor/Borsh layout, and reports how
//! many un-revoked (never `update_fee_shares_v2`'d) accounts have
//! admin == shareholders[0].address.
//!
//! Usage: cargo run [-- <rpc-url>]  (defaults to public mainnet-beta; pass a
//! private/paid RPC URL if the public endpoint rejects the unindexed
//! getProgramAccounts scan).

use solana_account_decoder::{UiAccountEncoding, UiDataSliceConfig};
use solana_program::pubkey::Pubkey;
use solana_rpc_client::rpc_client::RpcClient;
use solana_rpc_client_api::config::{
    RpcAccountInfoConfig, RpcProgramAccountsConfig, RpcTransactionConfig,
};
use solana_rpc_client_api::filter::{Memcmp, RpcFilterType};
use solana_transaction_status_client_types::{EncodedTransaction, UiMessage, UiTransactionEncoding};
use std::str::FromStr;
use std::time::Duration;

const CREATE_FEE_SHARING_CONFIG_DISCRIMINATOR: [u8; 8] = [195, 78, 86, 76, 111, 52, 251, 213];
const RESET_FEE_SHARING_CONFIG_DISCRIMINATOR: [u8; 8] = [10, 2, 182, 95, 16, 127, 129, 186];
const RESET_FEE_SHARING_CONFIG_V2_DISCRIMINATOR: [u8; 8] = [169, 245, 17, 209, 94, 91, 248, 128];

/// Per docs/plan/fees-03-instructions.md's `create_fee_sharing_config` account
/// table: event_authority(0), program(1), payer(2), global(3), mint(4),
/// sharing_config(5), ...
const CREATE_FEE_SHARING_CONFIG_PAYER_ACCOUNT_INDEX: usize = 2;

/// Covers discriminator(8) + bump(1) + version(1) + status(1) + mint(32) +
/// admin(32) + admin_revoked(1) + shareholders vec len(4) + shareholders[0]
/// (34) — everything `decode_sharing_config` reads, no further.
const DATA_SLICE_LEN: u64 = 8 + 1 + 1 + 1 + 32 + 32 + 1 + 4 + 34;

const PUMP_FEES_PROGRAM_ID: &str = "pfeeUxB6jkeY1Hxd7CsFCAjcbHA9rWtchMGdZ6VojVZ";
const SHARING_CONFIG_DISCRIMINATOR: [u8; 8] = [216, 74, 9, 0, 56, 140, 93, 75];

struct Shareholder {
    address: Pubkey,
    #[allow(dead_code)]
    share_bps: u16,
}

struct SharingConfig {
    bump: u8,
    version: u8,
    status: u8,
    mint: Pubkey,
    admin: Pubkey,
    admin_revoked: bool,
    shareholders_len: u32,
    /// Only the first entry — this probe never reads any other shareholder,
    /// so decoding stops here rather than requiring the full (possibly
    /// truncated, see `DATA_SLICE_LEN`) Vec to be present.
    first_shareholder: Option<Shareholder>,
}

fn decode_sharing_config(data: &[u8]) -> Option<SharingConfig> {
    if data.len() < 8 || data[0..8] != SHARING_CONFIG_DISCRIMINATOR {
        return None;
    }
    let mut o = 8usize;
    let bump = *data.get(o)?;
    o += 1;
    let version = *data.get(o)?;
    o += 1;
    let status = *data.get(o)?;
    o += 1;
    let mint = Pubkey::try_from(data.get(o..o + 32)?).ok()?;
    o += 32;
    let admin = Pubkey::try_from(data.get(o..o + 32)?).ok()?;
    o += 32;
    let admin_revoked = *data.get(o)? != 0;
    o += 1;

    let shareholders_len = u32::from_le_bytes(data.get(o..o + 4)?.try_into().ok()?);
    o += 4;

    let first_shareholder = if shareholders_len > 0 {
        let address = Pubkey::try_from(data.get(o..o + 32)?).ok()?;
        o += 32;
        let share_bps = u16::from_le_bytes(data.get(o..o + 2)?.try_into().ok()?);
        Some(Shareholder { address, share_bps })
    } else {
        None
    };

    Some(SharingConfig {
        bump,
        version,
        status,
        mint,
        admin,
        admin_revoked,
        shareholders_len,
        first_shareholder,
    })
}

struct HistoryFinding {
    creation_payer: Option<Pubkey>,
    saw_reset: bool,
    signature_count: usize,
    truncated: bool,
}

/// Walks `address`'s full transaction history (newest-first from the RPC,
/// processed oldest-first here) looking for: (a) the `payer` on its
/// `create_fee_sharing_config` call, and (b) whether `reset_fee_sharing_config`
/// or `_v2` ever appears — the two candidate explanations for why some
/// un-revoked `SharingConfig` accounts have `admin != shareholders[0]`.
fn investigate(client: &RpcClient, program_id: &Pubkey, address: &Pubkey) -> HistoryFinding {
    let sigs = client.get_signatures_for_address(address).unwrap_or_default();
    let truncated = sigs.len() >= 1000;
    let signature_count = sigs.len();

    let mut creation_payer = None;
    let mut saw_reset = false;

    for status in sigs.iter().rev() {
        let Ok(signature) = solana_signature::Signature::from_str(&status.signature) else {
            continue;
        };
        let config = RpcTransactionConfig {
            encoding: Some(UiTransactionEncoding::Json),
            max_supported_transaction_version: Some(0),
            ..Default::default()
        };
        let Ok(tx) = client.get_transaction_with_config(&signature, config) else {
            continue;
        };
        let EncodedTransaction::Json(ui_tx) = tx.transaction.transaction else {
            continue;
        };
        let UiMessage::Raw(msg) = ui_tx.message else {
            continue;
        };
        for ix in &msg.instructions {
            let Some(program_id_key) = msg.account_keys.get(ix.program_id_index as usize) else {
                continue;
            };
            if program_id_key != &program_id.to_string() {
                continue;
            }
            let Ok(data) = bs58::decode(&ix.data).into_vec() else {
                continue;
            };
            if data.len() < 8 {
                continue;
            }
            let disc: [u8; 8] = data[0..8].try_into().unwrap();
            if disc == RESET_FEE_SHARING_CONFIG_DISCRIMINATOR
                || disc == RESET_FEE_SHARING_CONFIG_V2_DISCRIMINATOR
            {
                saw_reset = true;
            }
            if disc == CREATE_FEE_SHARING_CONFIG_DISCRIMINATOR && creation_payer.is_none() {
                if let Some(payer_key_idx) =
                    ix.accounts.get(CREATE_FEE_SHARING_CONFIG_PAYER_ACCOUNT_INDEX)
                {
                    if let Some(payer_key_str) = msg.account_keys.get(*payer_key_idx as usize) {
                        creation_payer = Pubkey::from_str(payer_key_str).ok();
                    }
                }
            }
        }
    }

    HistoryFinding {
        creation_payer,
        saw_reset,
        signature_count,
        truncated,
    }
}

fn main() {
    let rpc_url = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "https://api.mainnet-beta.solana.com".to_string());
    println!("Connecting to {rpc_url}...");
    // Default client timeout is far too short for a 500k+-account scan even
    // with dataSlice trimming the payload down.
    let client = RpcClient::new_with_timeout(rpc_url, Duration::from_secs(300));

    let program_id = Pubkey::from_str(PUMP_FEES_PROGRAM_ID).unwrap();

    println!("Fetching SharingConfig accounts owned by {PUMP_FEES_PROGRAM_ID} (server-side discriminator filter, sliced to {DATA_SLICE_LEN} bytes/account)...");
    let config = RpcProgramAccountsConfig {
        filters: Some(vec![RpcFilterType::Memcmp(Memcmp::new_base58_encoded(
            0,
            &SHARING_CONFIG_DISCRIMINATOR,
        ))]),
        account_config: RpcAccountInfoConfig {
            encoding: Some(UiAccountEncoding::Base64),
            data_slice: Some(UiDataSliceConfig {
                offset: 0,
                length: DATA_SLICE_LEN as usize,
            }),
            ..Default::default()
        },
        ..Default::default()
    };
    // get_program_accounts_with_config is deprecated in this solana-rpc-client version and
    // returned 0 results despite the same discriminator matching 500k+ accounts in an earlier
    // unfiltered diagnostic run — using the replacement it names instead.
    let accounts = match client.get_program_ui_accounts_with_config(&program_id, config) {
        Ok(accounts) => accounts,
        Err(e) => {
            eprintln!("getProgramAccounts failed: {e:?}");
            eprintln!("(Pass a private/paid RPC URL as the first CLI arg if the public endpoint still rejects this: cargo run -- https://your-rpc-url)");
            std::process::exit(1);
        }
    };

    let mut total_unrevoked = 0;
    let mut matches = 0;
    let mut sharing_config_count = 0;
    let mut mismatched: Vec<(String, Pubkey)> = Vec::new();
    const MAX_PRINTED: usize = 30;

    for (address, account) in accounts {
        let Some(data) = account.data.decode() else {
            continue;
        };
        let Some(cfg) = decode_sharing_config(&data) else {
            continue;
        };
        sharing_config_count += 1;

        let first_shareholder_addr = cfg.first_shareholder.as_ref().map(|s| s.address);
        let is_match = !cfg.admin_revoked && first_shareholder_addr == Some(cfg.admin);

        if !cfg.admin_revoked {
            total_unrevoked += 1;
            if is_match {
                matches += 1;
            } else if mismatched.len() < 15 {
                mismatched.push((address.to_string(), cfg.admin));
            }
        }

        if sharing_config_count <= MAX_PRINTED {
            println!(
                "{}: mint={} admin={} admin_revoked={} shareholders_len={} shareholders[0]={:?} bump={} version={} status={}{}",
                address,
                cfg.mint,
                cfg.admin,
                cfg.admin_revoked,
                cfg.shareholders_len,
                first_shareholder_addr,
                cfg.bump,
                cfg.version,
                cfg.status,
                if is_match { "  <-- admin == shareholders[0]" } else { "" }
            );
        }
    }

    if sharing_config_count > MAX_PRINTED {
        println!("... ({} more not printed)", sharing_config_count - MAX_PRINTED);
    }
    println!("\nFound {sharing_config_count} SharingConfig account(s) total.");
    println!("{matches}/{total_unrevoked} un-revoked SharingConfig accounts have admin == shareholders[0].address.");

    println!(
        "\n--- Investigating {} mismatched accounts (admin != shareholders[0]) ---",
        mismatched.len()
    );
    for (address, admin) in &mismatched {
        let Ok(pubkey) = Pubkey::from_str(address) else {
            continue;
        };
        let finding = investigate(&client, &program_id, &pubkey);
        let payer_matches_admin = finding.creation_payer == Some(*admin);
        println!(
            "{address}: admin={admin} creation_payer={:?} payer==admin={payer_matches_admin} saw_reset_ix={} signatures_checked={}{}",
            finding.creation_payer,
            finding.saw_reset,
            finding.signature_count,
            if finding.truncated {
                " (TRUNCATED at 1000 sigs — oldest transaction may be missing)"
            } else {
                ""
            }
        );
    }
}
