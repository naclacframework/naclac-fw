//! Single-account lookup — no scanning. Usage:
//! cargo run --bin check_one_account -- <pubkey> [rpc-url]

use solana_program::pubkey::Pubkey;
use solana_rpc_client::rpc_client::RpcClient;
use std::str::FromStr;
use std::time::Duration;

fn main() {
    let mut args = std::env::args().skip(1);
    let key_str = args.next().expect("usage: check_one_account <pubkey> [rpc-url]");
    let rpc_url = args.next().unwrap_or_else(|| "https://api.mainnet-beta.solana.com".to_string());
    let client = RpcClient::new_with_timeout(rpc_url, Duration::from_secs(30));
    let key = Pubkey::from_str(&key_str).expect("invalid pubkey");

    match client.get_account(&key) {
        Ok(acc) => {
            println!("owner={}", acc.owner);
            println!("data_len={}", acc.data.len());
            println!("lamports={}", acc.lamports);
            println!("executable={}", acc.executable);
            if acc.data.len() >= 8 {
                println!("first 8 bytes (possible discriminator): {:02x?}", &acc.data[0..8]);
            }
            if acc.data.len() >= 72 && (acc.owner.to_string() == "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA" || acc.owner.to_string() == "TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb") {
                let mint = Pubkey::try_from(&acc.data[0..32]).unwrap();
                let owner = Pubkey::try_from(&acc.data[32..64]).unwrap();
                let amount = u64::from_le_bytes(acc.data[64..72].try_into().unwrap());
                println!("(looks like an SPL TokenAccount) mint={mint} owner={owner} amount={amount}");
            }
        }
        Err(e) => println!("fetch failed: {e:?}"),
    }
}
