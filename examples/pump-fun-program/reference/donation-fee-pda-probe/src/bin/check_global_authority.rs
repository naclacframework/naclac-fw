//! Checks what `Global.authority` actually is on real mainnet, and who
//! owns it — if it's a PDA owned by a known multisig program (Squads etc.),
//! the real setter for `buyback_fee_recipients` is very likely a CPI from
//! that multisig program into `pump`, which would be buried in
//! `meta.inner_instructions` and invisible to a top-level-instruction scan.

use solana_program::pubkey::Pubkey;
use solana_rpc_client::rpc_client::RpcClient;
use std::str::FromStr;
use std::time::Duration;

const PUMP_PROGRAM_ID: &str = "6EF8rrecthR5Dkzon8Nwu78hRvfCKubJ14M5uBEwF6P";

fn pk(s: &str) -> Pubkey {
    Pubkey::from_str(s).unwrap()
}

fn main() {
    let rpc_url = std::env::args().nth(1).unwrap_or_else(|| "https://api.mainnet-beta.solana.com".to_string());
    let client = RpcClient::new_with_timeout(rpc_url, Duration::from_secs(60));
    let pump_program = pk(PUMP_PROGRAM_ID);

    let (global, _) = Pubkey::find_program_address(&[b"global"], &pump_program);
    let account = client.get_account(&global).expect("failed to fetch global");

    // authority is at real offset 9..41 (disc(8) + initialized(1) + authority(32)).
    let authority_bytes = &account.data[9..41];
    let authority = Pubkey::try_from(authority_bytes).expect("valid pubkey");
    println!("Global.authority = {authority}");

    match client.get_account(&authority) {
        Ok(auth_account) => {
            println!("authority's owner program = {}", auth_account.owner);
            println!("authority's data length = {} bytes", auth_account.data.len());
            println!("authority is executable? {}", auth_account.executable);
            if auth_account.owner == pk("11111111111111111111111111111111") {
                println!("\n=> System-owned: this is a plain wallet (possibly a hardware wallet / single EOA), not a multisig PDA.");
            } else {
                println!("\n=> Owned by a program ({}), NOT a plain wallet -- consistent with a multisig/Squads-style vault.", auth_account.owner);
            }
        }
        Err(e) => println!("Failed to fetch authority account: {e:?} (may not exist / not yet funded)"),
    }
}
