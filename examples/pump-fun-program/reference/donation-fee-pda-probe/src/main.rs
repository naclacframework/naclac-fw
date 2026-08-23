//! Instead of guessing the exact validation logic behind
//! `create_donation_fee_pda`'s second `InvalidPool` check (probe13 in
//! `fee-tier-probe` exhausted 9 candidate PDA derivations against litesvm
//! without a match), this looks for REAL `DonationFeePda` accounts already
//! created on mainnet by the real program. If any exist, their stored
//! `creator`/`quote_mint`/`config_id` fields are ground truth — no need to
//! reverse-engineer the check that produced them. Also fetches each
//! account's correlated `bonding_curve`/`pool`/`sharing_config` to show
//! which field(s) `creator`/`quote_mint` actually came from.
//!
//! Usage: cargo run [-- <rpc-url>]  (defaults to public mainnet-beta).

use solana_account_decoder::UiAccountEncoding;
use solana_program::pubkey::Pubkey;
use solana_rpc_client::rpc_client::RpcClient;
use solana_rpc_client_api::config::{RpcAccountInfoConfig, RpcProgramAccountsConfig};
use solana_rpc_client_api::filter::{Memcmp, RpcFilterType};
use std::str::FromStr;
use std::time::Duration;

const PUMP_FEES_PROGRAM_ID: &str = "pfeeUxB6jkeY1Hxd7CsFCAjcbHA9rWtchMGdZ6VojVZ";
const PUMP_PROGRAM_ID: &str = "6EF8rrecthR5Dkzon8Nwu78hRvfCKubJ14M5uBEwF6P";
const PUMP_AMM_PROGRAM_ID: &str = "pAMMBay6oceH9fJKBRHGP5D4bD4sWpmSwMn52FMfXEA";
const WSOL_MINT_ID: &str = "So11111111111111111111111111111111111111112";

const DONATION_FEE_PDA_DISCRIMINATOR: [u8; 8] = [246, 197, 96, 9, 193, 30, 93, 115];
const BONDING_CURVE_DISCRIMINATOR: [u8; 8] = [23, 183, 248, 55, 96, 216, 172, 96];
const POOL_DISCRIMINATOR: [u8; 8] = [241, 154, 109, 4, 17, 177, 109, 188];
const SHARING_CONFIG_DISCRIMINATOR: [u8; 8] = [216, 74, 9, 0, 56, 140, 93, 75];

fn pk(s: &str) -> Pubkey {
    Pubkey::from_str(s).unwrap()
}

struct DonationFeePda {
    bump: u8,
    version: u8,
    config_id: Pubkey,
    base_mint: Pubkey,
    quote_mint: Pubkey,
    creator: Pubkey,
    total_donated: u64,
    last_crank_ts: i64,
}

fn decode_donation_fee_pda(data: &[u8]) -> Option<DonationFeePda> {
    if data.len() < 8 || data[0..8] != DONATION_FEE_PDA_DISCRIMINATOR {
        return None;
    }
    let mut o = 8usize;
    let bump = *data.get(o)?;
    o += 1;
    let version = *data.get(o)?;
    o += 1;
    let config_id = Pubkey::try_from(data.get(o..o + 32)?).ok()?;
    o += 32;
    let base_mint = Pubkey::try_from(data.get(o..o + 32)?).ok()?;
    o += 32;
    let quote_mint = Pubkey::try_from(data.get(o..o + 32)?).ok()?;
    o += 32;
    let creator = Pubkey::try_from(data.get(o..o + 32)?).ok()?;
    o += 32;
    let total_donated = u64::from_le_bytes(data.get(o..o + 8)?.try_into().ok()?);
    o += 8;
    let last_crank_ts = i64::from_le_bytes(data.get(o..o + 8)?.try_into().ok()?);

    Some(DonationFeePda {
        bump,
        version,
        config_id,
        base_mint,
        quote_mint,
        creator,
        total_donated,
        last_crank_ts,
    })
}

/// bonding_curve fields we care about here: skip 5 u64s + complete(bool),
/// then creator(pubkey), then is_mayhem_mode/is_cashback_coin(bool each),
/// then quote_mint(pubkey) — confirmed layout from pump-public-docs/idl/pump.json.
fn decode_bonding_curve(data: &[u8]) -> Option<(bool, Pubkey, Pubkey)> {
    if data.len() < 8 || data[0..8] != BONDING_CURVE_DISCRIMINATOR {
        return None;
    }
    let mut o = 8usize + 8 * 5;
    let complete = *data.get(o)? != 0;
    o += 1;
    let creator = Pubkey::try_from(data.get(o..o + 32)?).ok()?;
    o += 32 + 1 + 1;
    let quote_mint = Pubkey::try_from(data.get(o..o + 32)?).ok()?;
    Some((complete, creator, quote_mint))
}

/// pool_bump(1), index(2), creator(32), base_mint(32), quote_mint(32),
/// lp_mint(32), pool_base_token_account(32), pool_quote_token_account(32),
/// lp_supply(8), coin_creator(32) — confirmed from pump-public-docs/idl/pump_amm.json.
fn decode_pool(data: &[u8]) -> Option<(Pubkey, Pubkey, Pubkey, Pubkey)> {
    if data.len() < 8 || data[0..8] != POOL_DISCRIMINATOR {
        return None;
    }
    let mut o = 8usize + 1 + 2;
    let creator = Pubkey::try_from(data.get(o..o + 32)?).ok()?;
    o += 32;
    let base_mint = Pubkey::try_from(data.get(o..o + 32)?).ok()?;
    o += 32;
    let quote_mint = Pubkey::try_from(data.get(o..o + 32)?).ok()?;
    o += 32 + 32 + 32 + 32 + 8;
    let coin_creator = Pubkey::try_from(data.get(o..o + 32)?).ok()?;
    Some((creator, base_mint, quote_mint, coin_creator))
}

fn decode_sharing_config_admin(data: &[u8]) -> Option<(Pubkey, Pubkey)> {
    if data.len() < 8 || data[0..8] != SHARING_CONFIG_DISCRIMINATOR {
        return None;
    }
    let o = 8usize + 1 + 1 + 1;
    let mint = Pubkey::try_from(data.get(o..o + 32)?).ok()?;
    let admin = Pubkey::try_from(data.get(o + 32..o + 64)?).ok()?;
    Some((mint, admin))
}

fn main() {
    let rpc_url = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "https://api.mainnet-beta.solana.com".to_string());
    let client = RpcClient::new_with_timeout(rpc_url, Duration::from_secs(60));

    let fees_program = pk(PUMP_FEES_PROGRAM_ID);
    let pump_program = pk(PUMP_PROGRAM_ID);
    let pump_amm_program = pk(PUMP_AMM_PROGRAM_ID);
    let wsol_mint = pk(WSOL_MINT_ID);

    let config = RpcProgramAccountsConfig {
        filters: Some(vec![RpcFilterType::Memcmp(Memcmp::new_base58_encoded(
            0,
            &DONATION_FEE_PDA_DISCRIMINATOR,
        ))]),
        account_config: RpcAccountInfoConfig {
            encoding: Some(UiAccountEncoding::Base64),
            ..Default::default()
        },
        ..Default::default()
    };

    let accounts = match client.get_program_ui_accounts_with_config(&fees_program, config) {
        Ok(accounts) => accounts,
        Err(e) => {
            eprintln!("getProgramAccounts failed: {e:?}");
            eprintln!("(Pass a private/paid RPC URL as the first CLI arg if the public endpoint rejects this.)");
            std::process::exit(1);
        }
    };

    println!("Found {} DonationFeePda account(s) on mainnet.\n", accounts.len());

    for (address, account) in accounts {
        let Some(data) = account.data.decode() else { continue };
        let Some(dfp) = decode_donation_fee_pda(&data) else { continue };

        println!("=== DonationFeePda {address} ===");
        println!("  bump={} version={}", dfp.bump, dfp.version);
        println!("  config_id={}", dfp.config_id);
        println!("  base_mint={}", dfp.base_mint);
        println!("  quote_mint={}", dfp.quote_mint);
        println!("  creator={}", dfp.creator);
        println!("  total_donated={}", dfp.total_donated);
        println!("  last_crank_ts={}", dfp.last_crank_ts);

        let (bonding_curve, _) = Pubkey::find_program_address(
            &[b"bonding-curve", dfp.base_mint.as_ref()],
            &pump_program,
        );
        if let Ok(bc_account) = client.get_account(&bonding_curve) {
            if let Some((complete, bc_creator, bc_quote_mint)) = decode_bonding_curve(&bc_account.data) {
                println!(
                    "  bonding_curve: complete={complete} creator={bc_creator} (matches DonationFeePda.creator={}) quote_mint={bc_quote_mint} (matches DonationFeePda.quote_mint={})",
                    bc_creator == dfp.creator,
                    bc_quote_mint == dfp.quote_mint
                );
            }
        } else {
            println!("  bonding_curve: NOT FOUND at {bonding_curve}");
        }

        let (pool_authority, _) = Pubkey::find_program_address(
            &[b"pool-authority", dfp.base_mint.as_ref()],
            &pump_program,
        );
        let (pool, _) = Pubkey::find_program_address(
            &[b"pool", &0u16.to_le_bytes(), pool_authority.as_ref(), dfp.base_mint.as_ref(), wsol_mint.as_ref()],
            &pump_amm_program,
        );
        println!("  derived pool_authority={pool_authority} derived pool={pool}");
        if let Ok(pool_account) = client.get_account(&pool) {
            if let Some((p_creator, p_base_mint, p_quote_mint, p_coin_creator)) = decode_pool(&pool_account.data) {
                println!(
                    "  pool: creator={p_creator} (==pool_authority: {}) base_mint={p_base_mint} quote_mint={p_quote_mint} (matches DonationFeePda.quote_mint={}) coin_creator={p_coin_creator} (matches DonationFeePda.creator={})",
                    p_creator == pool_authority,
                    p_quote_mint == dfp.quote_mint,
                    p_coin_creator == dfp.creator
                );
            } else {
                println!("  pool account at derived address exists but doesn't decode as Pool (owner={})", pool_account.owner);
            }
        } else {
            println!("  pool: NOT FOUND at derived address {pool} (mint may not be graduated)");
        }

        let (sharing_config, _) = Pubkey::find_program_address(
            &[b"sharing-config", dfp.base_mint.as_ref()],
            &fees_program,
        );
        if let Ok(sc_account) = client.get_account(&sharing_config) {
            if let Some((sc_mint, sc_admin)) = decode_sharing_config_admin(&sc_account.data) {
                println!(
                    "  sharing_config: mint={sc_mint} admin={sc_admin} (matches DonationFeePda.creator={})",
                    sc_admin == dfp.creator
                );
            }
        } else {
            println!("  sharing_config: NOT FOUND at {sharing_config}");
        }

        println!();
    }
}
