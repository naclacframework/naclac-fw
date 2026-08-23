//! Confirms `reference/pump-rust-client/src/pda.rs`'s `bonding_curve_v2`
//! derivation (`find_program_address(&[b"bonding-curve-v2", mint], PROGRAM_ID)`)
//! against 3 known, real `(base_mint -> bonding_curve_v2)` pairs pulled
//! directly from real mainnet `buy`/`sell` transactions.

use solana_sdk::pubkey::Pubkey;
use std::str::FromStr;

const PUMP_PROGRAM_ID: &str = "6EF8rrecthR5Dkzon8Nwu78hRvfCKubJ14M5uBEwF6P";

fn pk(s: &str) -> Pubkey {
    Pubkey::from_str(s).unwrap()
}

fn main() {
    let program = pk(PUMP_PROGRAM_ID);
    let pairs = [
        ("GnvTKJ55We5UHrrgyh8Rcv9n5W2T7cxE12maCBNzpump", "HZNBgEykxAfoiwQenzZN6XJiTENNRZbMBmifwpxBSqzF"),
        ("G4CdaNdV2qqJV5C5tN32EoRajLHpLCNY5JitQTgApump", "5JFdn7waB7M8CssMcgPerKJaL59rY8g3GvpJa6xm8yCj"),
        ("8EuBPFvRnn4izzKa2KdtsAeBZcpJ7B4qFjUxzqpvpump", "J1HYZigeKebQebcDe8zzu75YNBDQjGPbPvnoXQDYZkmk"),
    ];

    for (mint_str, expected_str) in &pairs {
        let mint = pk(mint_str);
        let expected = pk(expected_str);
        let (derived, bump) = Pubkey::find_program_address(&[b"bonding-curve-v2", mint.as_ref()], &program);
        println!(
            "mint={mint_str} derived={derived} (bump={bump}) expected={expected_str} match={}",
            derived == expected
        );
    }
}
