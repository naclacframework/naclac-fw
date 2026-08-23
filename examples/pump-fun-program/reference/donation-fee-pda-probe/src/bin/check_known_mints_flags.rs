//! Reuses the mints already found by `sample_buyback_rotation` (no fresh
//! blind signature scan needed) and checks each one's `is_mayhem_mode`/
//! `is_cashback_coin` flags directly, to find a plain trade to
//! cross-reference next.
//!
//! Usage: cargo run --bin check_known_mints_flags [-- <rpc-url>]

use solana_program::pubkey::Pubkey;
use solana_rpc_client::rpc_client::RpcClient;
use std::str::FromStr;
use std::time::Duration;

const PUMP_PROGRAM_ID: &str = "6EF8rrecthR5Dkzon8Nwu78hRvfCKubJ14M5uBEwF6P";

// From sample_buyback_rotation's earlier successful run.
const KNOWN_MINTS: [&str; 8] = [
    "DXM8kkiv64GyjDnNLBmgPKGmGdmvL7ae21x2o1kp3Mvu",
    "AwKtugzkBqKC6b7d3JRTakDxuanSMjSg1XWmD7Qepump",
    "4GE1mwg92mwWGRPQrncwFsVp6GMsRcQQRiXtWzyBpump",
    "GNerfcf7efSY7iWYibMbsVLDZKNat4WDhkAkRGQbpump",
    "BN9tsWTVD89XJZ5j2nJRLfCSKwWmbSTxu6gJB3Jjpump",
    "LYjmUsnH33rCysboLnfzj6anEckN74wgP8GpxmztkAn",
    "GXPFM2caqTtQYC2cJ5yJRi9VDkpsYZXzYdwYpGnLmtDL", // (not actually a mint, harmless if it errors)
    "9SZvpw4sgLEEPEX1V7f726VbXsN3qWUin3S5cGsepump", // known mayhem-mode one, for comparison
];

fn pk(s: &str) -> Pubkey {
    Pubkey::from_str(s).unwrap()
}

fn main() {
    let rpc_url = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "https://api.mainnet-beta.solana.com".to_string());
    let client = RpcClient::new_with_timeout(rpc_url, Duration::from_secs(60));
    let pump_program = pk(PUMP_PROGRAM_ID);

    for mint_str in KNOWN_MINTS {
        let Ok(mint) = Pubkey::from_str(mint_str) else { continue };
        let (bonding_curve, _) = Pubkey::find_program_address(&[b"bonding-curve", mint.as_ref()], &pump_program);
        match client.get_account(&bonding_curve) {
            Ok(account) if account.data.len() >= 8 + 40 + 1 + 32 + 2 => {
                let is_mayhem_mode = account.data[8 + 40 + 1 + 32];
                let is_cashback_coin = account.data[8 + 40 + 1 + 32 + 1];
                let plain = if is_mayhem_mode == 0 && is_cashback_coin == 0 { " <- PLAIN" } else { "" };
                println!("mint={mint_str} is_mayhem_mode={is_mayhem_mode} is_cashback_coin={is_cashback_coin}{plain}");
            }
            Ok(_) => println!("mint={mint_str}: bonding_curve too short/not found"),
            Err(e) => println!("mint={mint_str}: fetch failed: {e:?}"),
        }
    }
}
