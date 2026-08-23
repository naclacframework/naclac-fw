//! Isolates whether `naclac_core::prelude::derive_program_address`'s formula
//! itself is correct, independent of any naclac-side plumbing (Address type
//! conversions, scratch-array logic, etc.) — pure `solana-sdk` only, no
//! naclac dependency at all. Compares real `Pubkey::find_program_address`
//! against a byte-for-byte copy of the exact formula now in
//! `naclac-core/src/prelude.rs`.

use solana_sdk::pubkey::Pubkey;
use solana_sdk::hash::hashv;
use std::str::FromStr;

// Byte-for-byte copy of `derive_program_address`'s formula (the
// non-pinocchio branch), for isolated comparison only.
fn derive_program_address_copy(seeds: &[&[u8]], bump: u8, program_id: &Pubkey) -> Pubkey {
    const MAX_PDA_SEEDS: usize = 16;
    assert!(seeds.len() <= MAX_PDA_SEEDS);
    let bump_arr = [bump];
    let mut scratch: [&[u8]; MAX_PDA_SEEDS + 3] = [&[]; MAX_PDA_SEEDS + 3];
    let mut n = 0;
    for seed in seeds {
        scratch[n] = seed;
        n += 1;
    }
    scratch[n] = &bump_arr[..];
    n += 1;
    scratch[n] = program_id.as_ref();
    n += 1;
    scratch[n] = b"ProgramDerivedAddress";
    n += 1;
    let inputs = &scratch[..n];
    let hash_result = hashv(inputs);
    Pubkey::new_from_array(hash_result.to_bytes())
}

fn main() {
    let pump_fees_program = Pubkey::from_str("8rG6Zs43yJ71tkCsWoEqdxF1uN9HzpQnCS8huqkKuwPJ").unwrap();
    let seed = b"buyback-vault";

    for i in 0u8..8 {
        let index_seed = [i];
        let seeds: &[&[u8]] = &[seed, &index_seed];
        let (real_pda, real_bump) = Pubkey::find_program_address(seeds, &pump_fees_program);

        // Sanity check: does create_program_address with the found bump
        // reproduce find_program_address's own result?
        let cpa_seeds: Vec<&[u8]> = vec![seed, &index_seed, std::slice::from_ref(&real_bump)];
        let cpa_result = Pubkey::create_program_address(&cpa_seeds, &pump_fees_program).unwrap();
        let cpa_matches = cpa_result == real_pda;

        let my_result = derive_program_address_copy(seeds, real_bump, &pump_fees_program);
        let my_matches = my_result == real_pda;

        println!(
            "index={i} bump={real_bump} real_pda={real_pda} create_program_address_matches={cpa_matches} my_formula_matches={my_matches}"
        );
        if !my_matches {
            println!("  MISMATCH: my_result={my_result}");
        }
    }
}
