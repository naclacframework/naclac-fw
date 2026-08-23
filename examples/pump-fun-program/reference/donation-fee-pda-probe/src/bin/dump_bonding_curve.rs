//! Real `bonding_curve` accounts are 151 bytes, but the documented IDL
//! fields only account for ~107 (5*u64 + complete + creator + 2*bool +
//! quote_mint) — a 36-byte gap, same "undocumented extra fields" pattern
//! already found in `Global`/`UserVolumeAccumulator`. Dumping the full raw
//! bytes to look for anything resembling a buyback-rotation counter, rather
//! than guess.
//!
//! Usage: cargo run --bin dump_bonding_curve -- <mint> [rpc-url]

use solana_program::pubkey::Pubkey;
use solana_rpc_client::rpc_client::RpcClient;
use std::str::FromStr;
use std::time::Duration;

const PUMP_PROGRAM_ID: &str = "6EF8rrecthR5Dkzon8Nwu78hRvfCKubJ14M5uBEwF6P";

fn main() {
    let mint_str = std::env::args().nth(1).expect("usage: dump_bonding_curve <mint> [rpc-url]");
    let mint = Pubkey::from_str(&mint_str).expect("invalid mint pubkey");
    let rpc_url = std::env::args()
        .nth(2)
        .unwrap_or_else(|| "https://api.mainnet-beta.solana.com".to_string());
    let client = RpcClient::new_with_timeout(rpc_url, Duration::from_secs(60));

    let pump_program = Pubkey::from_str(PUMP_PROGRAM_ID).unwrap();
    let (bonding_curve, _) = Pubkey::find_program_address(&[b"bonding-curve", mint.as_ref()], &pump_program);

    println!("bonding_curve = {bonding_curve}");
    let account = client.get_account(&bonding_curve).expect("failed to fetch bonding_curve");
    println!("total bytes: {}", account.data.len());
    println!("raw: {:02x?}", account.data);

    // Known/documented layout up to quote_mint.
    let mut o = 8usize;
    let virtual_token_reserves = u64::from_le_bytes(account.data[o..o+8].try_into().unwrap()); o += 8;
    let virtual_sol_reserves = u64::from_le_bytes(account.data[o..o+8].try_into().unwrap()); o += 8;
    let real_token_reserves = u64::from_le_bytes(account.data[o..o+8].try_into().unwrap()); o += 8;
    let real_sol_reserves = u64::from_le_bytes(account.data[o..o+8].try_into().unwrap()); o += 8;
    let token_total_supply = u64::from_le_bytes(account.data[o..o+8].try_into().unwrap()); o += 8;
    let complete = account.data[o]; o += 1;
    let creator = Pubkey::try_from(&account.data[o..o+32]).unwrap(); o += 32;
    let is_mayhem_mode = account.data[o]; o += 1;
    let is_cashback_coin = account.data[o]; o += 1;
    let quote_mint = Pubkey::try_from(&account.data[o..o+32]).unwrap(); o += 32;

    println!("\nDocumented fields:");
    println!("  virtual_token_reserves = {virtual_token_reserves}");
    println!("  virtual_sol_reserves = {virtual_sol_reserves}");
    println!("  real_token_reserves = {real_token_reserves}");
    println!("  real_sol_reserves = {real_sol_reserves}");
    println!("  token_total_supply = {token_total_supply}");
    println!("  complete = {complete}");
    println!("  creator = {creator}");
    println!("  is_mayhem_mode = {is_mayhem_mode}");
    println!("  is_cashback_coin = {is_cashback_coin}");
    println!("  quote_mint = {quote_mint}");
    println!("\nBytes consumed so far: {o} / {}", account.data.len());
    println!("Undocumented trailing bytes ({} bytes): {:02x?}", account.data.len() - o, &account.data[o..]);
}
