//! probe13 (fee-tier-probe) confirmed `create_donation_fee_pda`'s `pool`
//! account must deserialize as a real `Account<Pool>` (AccountDiscriminatorMismatch
//! otherwise) — so for the 15 real, ungraduated (bonding_curve.complete=false)
//! `DonationFeePda`s found by `main.rs`, SOME real Pool-shaped account must
//! exist for their base_mint, at an address our `pool_authority`-chained
//! seed formula doesn't predict (that derived address came back NOT FOUND).
//! This searches pump_amm_program's accounts directly by content
//! (discriminator + base_mint match) instead of guessing the address.
//!
//! Usage: cargo run --bin find_pool -- <base_mint> [rpc-url]

use solana_account_decoder::UiAccountEncoding;
use solana_program::pubkey::Pubkey;
use solana_rpc_client::rpc_client::RpcClient;
use solana_rpc_client_api::config::{RpcAccountInfoConfig, RpcProgramAccountsConfig};
use solana_rpc_client_api::filter::{Memcmp, RpcFilterType};
use std::str::FromStr;
use std::time::Duration;

const PUMP_AMM_PROGRAM_ID: &str = "pAMMBay6oceH9fJKBRHGP5D4bD4sWpmSwMn52FMfXEA";
const POOL_DISCRIMINATOR: [u8; 8] = [241, 154, 109, 4, 17, 177, 109, 188];
// discriminator(8) + pool_bump(1) + index(2) + creator(32) = 43
const BASE_MINT_OFFSET: usize = 43;

fn main() {
    let base_mint_str = std::env::args()
        .nth(1)
        .expect("usage: find_pool <base_mint> [rpc-url]");
    let base_mint = Pubkey::from_str(&base_mint_str).expect("invalid base_mint pubkey");
    let rpc_url = std::env::args()
        .nth(2)
        .unwrap_or_else(|| "https://api.mainnet-beta.solana.com".to_string());
    let client = RpcClient::new_with_timeout(rpc_url, Duration::from_secs(60));

    let pump_amm_program = Pubkey::from_str(PUMP_AMM_PROGRAM_ID).unwrap();

    let config = RpcProgramAccountsConfig {
        filters: Some(vec![
            RpcFilterType::Memcmp(Memcmp::new_base58_encoded(0, &POOL_DISCRIMINATOR)),
            RpcFilterType::Memcmp(Memcmp::new_base58_encoded(BASE_MINT_OFFSET, base_mint.as_ref())),
        ]),
        account_config: RpcAccountInfoConfig {
            encoding: Some(UiAccountEncoding::Base64),
            ..Default::default()
        },
        ..Default::default()
    };

    let accounts = match client.get_program_ui_accounts_with_config(&pump_amm_program, config) {
        Ok(accounts) => accounts,
        Err(e) => {
            eprintln!("getProgramAccounts failed: {e:?}");
            std::process::exit(1);
        }
    };

    println!("Found {} Pool account(s) for base_mint={base_mint}", accounts.len());
    for (address, account) in accounts {
        let Some(data) = account.data.decode() else { continue };
        println!("\n=== Pool {address} ===");
        println!("  raw data ({} bytes): {:02x?}", data.len(), data);
        if data.len() >= 8 + 1 + 2 + 32 * 8 + 8 {
            let mut o = 8usize;
            let pool_bump = data[o]; o += 1;
            let index = u16::from_le_bytes(data[o..o + 2].try_into().unwrap()); o += 2;
            let creator = Pubkey::try_from(&data[o..o + 32]).unwrap(); o += 32;
            let got_base_mint = Pubkey::try_from(&data[o..o + 32]).unwrap(); o += 32;
            let quote_mint = Pubkey::try_from(&data[o..o + 32]).unwrap(); o += 32;
            let lp_mint = Pubkey::try_from(&data[o..o + 32]).unwrap(); o += 32;
            let pool_base_token_account = Pubkey::try_from(&data[o..o + 32]).unwrap(); o += 32;
            let pool_quote_token_account = Pubkey::try_from(&data[o..o + 32]).unwrap(); o += 32;
            let lp_supply = u64::from_le_bytes(data[o..o + 8].try_into().unwrap()); o += 8;
            let coin_creator = Pubkey::try_from(&data[o..o + 32]).unwrap();
            println!("  pool_bump={pool_bump} index={index}");
            println!("  creator={creator}");
            println!("  base_mint={got_base_mint}");
            println!("  quote_mint={quote_mint}");
            println!("  lp_mint={lp_mint}");
            println!("  pool_base_token_account={pool_base_token_account}");
            println!("  pool_quote_token_account={pool_quote_token_account}");
            println!("  lp_supply={lp_supply}");
            println!("  coin_creator={coin_creator}");
        }
    }
}
