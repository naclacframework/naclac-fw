//! Decodes the real `pump_amm` self-CPI event logs nested inside a known,
//! successful `migrate_v2` transaction's inner instructions -- specifically
//! the ones at stack height 3 with a single `event_authority` account,
//! which `probe45.rs`'s size-filtered scan (>= 13 accounts) skipped over.
//! One of these (following the `init_boost` CPI) should be the real
//! `InitBoostEvent`, needed to settle how `pool.virtual_quote_reserves`
//! actually gets set -- `probe47.rs` proved standalone `init_boost` does NOT
//! derive it from `boost_vault`'s balance the way earlier analysis assumed.
//!
//! Usage: cargo run --bin decode_boost_events [-- <rpc-url>]

use solana_rpc_client::rpc_client::RpcClient;
use solana_rpc_client_api::config::RpcTransactionConfig;
use solana_transaction_status_client_types::{
    EncodedTransaction, UiInstruction, UiMessage, UiTransactionEncoding,
};
use std::str::FromStr;
use std::time::Duration;

const DEFAULT_SIGNATURE: &str = "2buaKvMn3vPH6DfnVeaMCv56PbVC31hQPuRETDL17VrpX3HgB7TwcxzdyH8Qh4Nw8qr8fxW2asoZgtbVgHNsBU51";
const PUMP_AMM_PROGRAM_ID: &str = "pAMMBay6oceH9fJKBRHGP5D4bD4sWpmSwMn52FMfXEA";

fn b58_pubkey(bytes: &[u8]) -> String {
    bs58::encode(bytes).into_string()
}

fn main() {
    let sig_arg = std::env::args().nth(1).unwrap_or_else(|| DEFAULT_SIGNATURE.to_string());
    let rpc_url = std::env::args().nth(2).unwrap_or_else(|| "https://api.mainnet-beta.solana.com".to_string());
    let client = RpcClient::new_with_timeout(rpc_url, Duration::from_secs(60));

    println!("=== signature: {sig_arg} ===");
    let signature = solana_signature::Signature::from_str(&sig_arg).expect("valid signature");
    let tx = client
        .get_transaction_with_config(
            &signature,
            RpcTransactionConfig { encoding: Some(UiTransactionEncoding::Json), commitment: None, max_supported_transaction_version: Some(0) },
        )
        .expect("fetch tx failed");

    let EncodedTransaction::Json(ui_tx) = &tx.transaction.transaction else { panic!("not JSON") };
    let UiMessage::Raw(raw_msg) = &ui_tx.message else { panic!("not raw message") };
    let mut all_keys: Vec<String> = raw_msg.account_keys.clone();
    let meta = tx.transaction.meta.as_ref().expect("meta missing");
    if let solana_transaction_status_client_types::option_serializer::OptionSerializer::Some(loaded) = &meta.loaded_addresses {
        all_keys.extend(loaded.writable.iter().cloned());
        all_keys.extend(loaded.readonly.iter().cloned());
    }
    let pump_amm_str = PUMP_AMM_PROGRAM_ID.to_string();

    let solana_transaction_status_client_types::option_serializer::OptionSerializer::Some(inner_list) = &meta.inner_instructions else {
        panic!("no inner instructions");
    };

    let mut event_num = 0;
    for group in inner_list {
        for inner_ix in &group.instructions {
            let UiInstruction::Compiled(compiled) = inner_ix else { continue };
            let prog = all_keys.get(compiled.program_id_index as usize);
            if prog != Some(&pump_amm_str) {
                continue;
            }
            // Self-CPI event logs: exactly one account (event_authority).
            if compiled.accounts.len() != 1 {
                continue;
            }
            let Ok(data) = bs58::decode(&compiled.data).into_vec() else { continue };
            event_num += 1;
            println!("\n=== pump_amm self-CPI event #{event_num} (raw {} bytes) ===", data.len());
            if data.len() < 16 {
                println!("(too short)");
                continue;
            }
            // First 8 bytes: Anchor's generic self-CPI "event" wrapper
            // discriminator (identical across every event type). The
            // event-type-specific discriminator is the *next* 8 bytes --
            // dispatch on THAT, then decode each event type with its own
            // real field layout (from pump_amm.json's `types[]`) rather than
            // assuming a shared prefix across different event structs, which
            // previously produced garbage for CreatePoolEvent by misapplying
            // InitBoostEvent's (timestamp, mint, bonding_curve, pool) shape.
            let outer_disc = &data[0..8];
            let event_disc = &data[8..16];
            println!("outer (generic) discriminator: {outer_disc:?}");
            println!("event-type discriminator: {event_disc:?}");
            let mut off = 16usize;

            const INIT_BOOST_EVENT_DISC: [u8; 8] = [174, 124, 74, 249, 4, 81, 246, 17];
            const CREATE_POOL_EVENT_DISC: [u8; 8] = [177, 49, 12, 210, 160, 118, 167, 116];

            if event_disc == INIT_BOOST_EVENT_DISC {
                // InitBoostEvent: timestamp(8) + mint(32) + bonding_curve(32)
                // + pool(32) + virtual_quote_reserves(i128,16) + real_quote_reserves_after(u64,8)
                let timestamp = i64::from_le_bytes(data[off..off + 8].try_into().unwrap()); off += 8;
                let mint = b58_pubkey(&data[off..off + 32]); off += 32;
                let bonding_curve = b58_pubkey(&data[off..off + 32]); off += 32;
                let pool = b58_pubkey(&data[off..off + 32]); off += 32;
                let vqr = i128::from_le_bytes(data[off..off + 16].try_into().unwrap()); off += 16;
                let real_quote_reserves_after = u64::from_le_bytes(data[off..off + 8].try_into().unwrap()); off += 8;
                println!("--> InitBoostEvent:");
                println!("    timestamp: {timestamp}");
                println!("    mint: {mint}");
                println!("    bonding_curve: {bonding_curve}");
                println!("    pool: {pool}");
                println!("    virtual_quote_reserves: {vqr}");
                println!("    real_quote_reserves_after: {real_quote_reserves_after}");
                println!("    bytes consumed: {off} of {}", data.len());
            } else if event_disc == CREATE_POOL_EVENT_DISC {
                // CreatePoolEvent: timestamp(8) + index(u16,2) + creator(32)
                // + base_mint(32) + quote_mint(32) + base_mint_decimals(1)
                // + quote_mint_decimals(1) + base_amount_in(8) + quote_amount_in(8)
                // + pool_base_amount(8) + pool_quote_amount(8) + minimum_liquidity(8)
                // + initial_liquidity(8) + lp_token_amount_out(8) + pool_bump(1)
                // + pool(32) + lp_mint(32) + user_base_token_account(32)
                // + user_quote_token_account(32) + coin_creator(32) + is_mayhem_mode(1)
                let timestamp = i64::from_le_bytes(data[off..off + 8].try_into().unwrap()); off += 8;
                let index = u16::from_le_bytes(data[off..off + 2].try_into().unwrap()); off += 2;
                let creator = b58_pubkey(&data[off..off + 32]); off += 32;
                let base_mint = b58_pubkey(&data[off..off + 32]); off += 32;
                let quote_mint = b58_pubkey(&data[off..off + 32]); off += 32;
                let base_mint_decimals = data[off]; off += 1;
                let quote_mint_decimals = data[off]; off += 1;
                let base_amount_in = u64::from_le_bytes(data[off..off + 8].try_into().unwrap()); off += 8;
                let quote_amount_in = u64::from_le_bytes(data[off..off + 8].try_into().unwrap()); off += 8;
                let pool_base_amount = u64::from_le_bytes(data[off..off + 8].try_into().unwrap()); off += 8;
                let pool_quote_amount = u64::from_le_bytes(data[off..off + 8].try_into().unwrap()); off += 8;
                let minimum_liquidity = u64::from_le_bytes(data[off..off + 8].try_into().unwrap()); off += 8;
                let initial_liquidity = u64::from_le_bytes(data[off..off + 8].try_into().unwrap()); off += 8;
                let lp_token_amount_out = u64::from_le_bytes(data[off..off + 8].try_into().unwrap()); off += 8;
                let pool_bump = data[off]; off += 1;
                let pool_field = b58_pubkey(&data[off..off + 32]); off += 32;
                let lp_mint = b58_pubkey(&data[off..off + 32]); off += 32;
                let user_base_token_account = b58_pubkey(&data[off..off + 32]); off += 32;
                let user_quote_token_account = b58_pubkey(&data[off..off + 32]); off += 32;
                let coin_creator = b58_pubkey(&data[off..off + 32]); off += 32;
                let is_mayhem_mode = data[off]; off += 1;
                println!("--> CreatePoolEvent:");
                println!("    timestamp: {timestamp}");
                println!("    index: {index}");
                println!("    creator: {creator}");
                println!("    base_mint: {base_mint}");
                println!("    quote_mint: {quote_mint}");
                println!("    base_mint_decimals: {base_mint_decimals}");
                println!("    quote_mint_decimals: {quote_mint_decimals}");
                println!("    base_amount_in: {base_amount_in}");
                println!("    quote_amount_in: {quote_amount_in}");
                println!("    pool_base_amount: {pool_base_amount}");
                println!("    pool_quote_amount: {pool_quote_amount}");
                println!("    minimum_liquidity: {minimum_liquidity}");
                println!("    initial_liquidity: {initial_liquidity}");
                println!("    lp_token_amount_out: {lp_token_amount_out}");
                println!("    pool_bump: {pool_bump}");
                println!("    pool: {pool_field}");
                println!("    lp_mint: {lp_mint}");
                println!("    user_base_token_account: {user_base_token_account}");
                println!("    user_quote_token_account: {user_quote_token_account}");
                println!("    coin_creator: {coin_creator}");
                println!("    is_mayhem_mode: {is_mayhem_mode}");
                println!("    bytes consumed: {off} of {}", data.len());
            } else {
                println!("(unknown event discriminator, dumping raw hex)");
                println!("{}", data[off..].iter().map(|b| format!("{b:02x}")).collect::<String>());
            }
        }
    }
    println!("\ntotal pump_amm self-CPI events found: {event_num}");
}
