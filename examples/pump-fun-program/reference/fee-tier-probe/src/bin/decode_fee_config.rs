//! Fetches the real `pump_amm` GlobalConfig and prints its fee-bps fields
//! precisely, to settle (not guess) whether boost_buy_and_burn's small
//! observed pricing residual (~0.0002%) is actually explained by LP/protocol
//! fee deduction before the swap, rather than assuming it without checking.
//!
//! Usage: cargo run --bin decode_fee_config [-- <rpc-url>]

use solana_rpc_client::rpc_client::RpcClient;
use solana_sdk::pubkey::Pubkey;
use std::str::FromStr;
use std::time::Duration;

const PUMP_AMM_PROGRAM_ID: &str = "pAMMBay6oceH9fJKBRHGP5D4bD4sWpmSwMn52FMfXEA";

fn main() {
    let rpc_url = std::env::args().nth(1).unwrap_or_else(|| "https://api.mainnet-beta.solana.com".to_string());
    let client = RpcClient::new_with_timeout(rpc_url, Duration::from_secs(60));
    let pump_amm_program = Pubkey::from_str(PUMP_AMM_PROGRAM_ID).unwrap();
    let (amm_global_config, _) = Pubkey::find_program_address(&[b"global_config"], &pump_amm_program);
    println!("amm_global_config address = {amm_global_config}");

    let account = client.get_account(&amm_global_config).expect("fetch failed");
    let data = &account.data;
    println!("total data len = {}", data.len());

    let admin = Pubkey::try_from(&data[8..40]).unwrap();
    let lp_fee_basis_points = u64::from_le_bytes(data[40..48].try_into().unwrap());
    let protocol_fee_basis_points = u64::from_le_bytes(data[48..56].try_into().unwrap());
    let disable_flags = data[56];
    let coin_creator_fee_basis_points = u64::from_le_bytes(data[313..321].try_into().unwrap());
    let buyback_basis_points = u64::from_le_bytes(data[899..907].try_into().unwrap());
    let boost_authority = Pubkey::try_from(&data[907..939]).unwrap();
    let boost_enabled = data[939];

    println!("admin = {admin}");
    println!("lp_fee_basis_points = {lp_fee_basis_points}");
    println!("protocol_fee_basis_points = {protocol_fee_basis_points}");
    println!("disable_flags = {disable_flags}");
    println!("coin_creator_fee_basis_points = {coin_creator_fee_basis_points}");
    println!("buyback_basis_points = {buyback_basis_points}");
    println!("boost_authority = {boost_authority}");
    println!("boost_enabled = {boost_enabled}");
}
