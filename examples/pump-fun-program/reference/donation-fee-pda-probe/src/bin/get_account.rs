//! Fetches raw getAccountInfo for one address — used to check exactly what
//! (if anything) exists at the `pool` address a real CreateDonationFeePda
//! transaction passed, since a content-based search (find_pool) found no
//! Pool-discriminated account for that mint anywhere.
//!
//! Usage: cargo run --bin get_account -- <address> [rpc-url]

use solana_program::pubkey::Pubkey;
use solana_rpc_client::rpc_client::RpcClient;
use std::str::FromStr;
use std::time::Duration;

fn main() {
    let address_str = std::env::args().nth(1).expect("usage: get_account <address> [rpc-url]");
    let address = Pubkey::from_str(&address_str).expect("invalid pubkey");
    let rpc_url = std::env::args()
        .nth(2)
        .unwrap_or_else(|| "https://api.mainnet-beta.solana.com".to_string());
    let client = RpcClient::new_with_timeout(rpc_url, Duration::from_secs(60));

    match client.get_account(&address) {
        Ok(account) => {
            println!("EXISTS: {address}");
            println!("  owner = {}", account.owner);
            println!("  lamports = {}", account.lamports);
            println!("  executable = {}", account.executable);
            println!("  data len = {}", account.data.len());
            if account.data.len() >= 8 {
                println!("  discriminator = {:?}", &account.data[0..8]);
            }
            println!("  first 64 bytes = {:02x?}", &account.data[..account.data.len().min(64)]);
        }
        Err(e) => {
            println!("NOT FOUND / error fetching {address}: {e:?}");
        }
    }
}
