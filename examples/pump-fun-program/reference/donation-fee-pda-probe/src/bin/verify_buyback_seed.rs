//! `probe16` still fails `BuybackFeeRecipientMissing` even with a real
//! `BuybackVault`-shaped fixture at `find_program_address([b"buyback-vault",
//! &[index]], pump_fees_program)`. Before probing further blind: does that
//! seed formula even reproduce the REAL address already confirmed at
//! `global.buyback_fee_recipients[4]` (`5cjcW9wExnJJiqgLjq7DEG75Pm6JBgE1hNv4B2vHXUW6`,
//! from `inspect_buy_tx`)? If not, the seed constant itself is wrong.
//!
//! Usage: cargo run --bin verify_buyback_seed

use solana_program::pubkey::Pubkey;
use std::str::FromStr;

const PUMP_FEES_PROGRAM_ID: &str = "pfeeUxB6jkeY1Hxd7CsFCAjcbHA9rWtchMGdZ6VojVZ";
const KNOWN_REAL_VAULT_AT_INDEX_4: &str = "5cjcW9wExnJJiqgLjq7DEG75Pm6JBgE1hNv4B2vHXUW6";

fn main() {
    let pump_fees_program = Pubkey::from_str(PUMP_FEES_PROGRAM_ID).unwrap();
    let known = Pubkey::from_str(KNOWN_REAL_VAULT_AT_INDEX_4).unwrap();

    println!("Known real global.buyback_fee_recipients[4] = {known}\n");

    for index in 0u16..16 {
        let idx_u8 = index as u8;
        let (candidate_u8, _) = Pubkey::find_program_address(&[b"buyback-vault", &[idx_u8]], &pump_fees_program);
        let m1 = candidate_u8 == known;
        let (candidate_no_dash, _) = Pubkey::find_program_address(&[b"buyback_vault", &[idx_u8]], &pump_fees_program);
        let m3 = candidate_no_dash == known;
        if m1 || m3 {
            println!("MATCH at index={index}: [b\"buyback-vault\"/\"buyback_vault\", index] -> {known}");
        }
        println!(
            "  index={index}: [b\"buyback-vault\",[u8]]={candidate_u8} match={m1} | [b\"buyback_vault\",[u8]]={candidate_no_dash} match={m3}"
        );
    }
}
