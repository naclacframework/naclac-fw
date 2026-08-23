//! Resolves fees-06-open-questions.md #2's residual gap: which table does
//! `get_fees` actually read for `is_pump_pool`/`is_new_quote_mint`, now that
//! `fee_tiers`/`stable_fee_tiers` are identical on real mainnet state and
//! can't be told apart by output alone?
//!
//! Strategy: clone the REAL fee_config bytes (so `fee_tiers[0]` keeps its
//! real `{lp:0, protocol:95, creator:30}`), then patch ONLY
//! `stable_fee_tiers[0]`'s `Fees` bytes in-memory to a value that could never
//! occur on the real deployed account (`{lp:11, protocol:22, creator:33}`).
//! Both tiers keep `threshold=0`, so whichever table is picked, its single
//! tier always matches for any market cap >= 0 — the returned `Fees` is then
//! an unambiguous fingerprint of which table `get_fees` actually read.

use litesvm::LiteSVM;
use solana_sdk::{
    account::Account,
    instruction::{AccountMeta, Instruction},
    message::Message,
    pubkey::Pubkey,
    signature::{Keypair, Signer},
    transaction::Transaction,
};
use std::str::FromStr;

const PUMP_FEES_PROGRAM_ID: &str = "pfeeUxB6jkeY1Hxd7CsFCAjcbHA9rWtchMGdZ6VojVZ";
const PUMP_PROGRAM_ID: &str = "6EF8rrecthR5Dkzon8Nwu78hRvfCKubJ14M5uBEwF6P";
const FEE_CONFIG: &str = "8Wf5TiAheLUqBrKXeYg2JtAFFMWtKdG2BSFgqUcPVwTt";
const GET_FEES_DISCRIMINATOR: [u8; 8] = [231, 37, 126, 85, 207, 91, 63, 52];

// Layout: 8 (disc) + 1 (bump) + 32 (admin) + 24 (flat_fees) + fee_tiers +
// stable_fee_tiers. Both `Vec<FeeTier>` fields are plain compact Borsh (4-byte
// len + len x 40-byte entries) — NOT pre-padded to MAX_FEE_TIERS=50 slots.
// fees-06#11's 4073-byte account size comes from `extend_fee_config`'s realloc
// reserving trailing capacity AFTER the serialized struct, not from padding
// between fields — confirmed empirically below (this constant assumed the
// wrong, padded layout on the first attempt; the sanity assert caught it).
const FLAT_FEES_START: usize = 41;
const FEE_TIERS_VEC_START: usize = 65;
const FEE_TIERS_LEN: usize = 1; // real on-chain state per fees-06#2, at time of writing
const STABLE_FEE_TIERS_VEC_START: usize = FEE_TIERS_VEC_START + 4 + FEE_TIERS_LEN * 40; // 109
const STABLE_FEE_TIERS_0_FEES_START: usize = STABLE_FEE_TIERS_VEC_START + 4 + 16; // 129

fn pk(s: &str) -> Pubkey {
    Pubkey::from_str(s).unwrap()
}

fn build_get_fees_ix(
    program_id: Pubkey,
    fee_config: Pubkey,
    config_program_id: Pubkey,
    is_pump_pool: bool,
    market_cap_lamports: u128,
    is_new_quote_mint: bool,
) -> Instruction {
    let mut data = Vec::with_capacity(8 + 1 + 16 + 8 + 1);
    data.extend_from_slice(&GET_FEES_DISCRIMINATOR);
    data.push(is_pump_pool as u8);
    data.extend_from_slice(&market_cap_lamports.to_le_bytes());
    data.extend_from_slice(&0u64.to_le_bytes()); // trade_size_lamports, unused per fees-04 §3
    data.push(is_new_quote_mint as u8);
    Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new_readonly(fee_config, false),
            AccountMeta::new_readonly(config_program_id, false),
        ],
        data,
    }
}

fn main() {
    let program_id = pk(PUMP_FEES_PROGRAM_ID);
    let config_program_id = pk(PUMP_PROGRAM_ID);
    let fee_config = pk(FEE_CONFIG);

    let mut svm = LiteSVM::new();
    let so_path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../pump-rust-client/artifacts/pump_fees.so"
    );
    svm.add_program_from_file(program_id, so_path)
        .expect("load pump_fees.so");

    let mut data = include_bytes!("../../fixtures/fee_config.bin").to_vec();

    // Sanity-check we're patching where we think we are: real stable_fee_tiers[0]
    // should currently equal fee_tiers[0] = {0, 95, 30} before the patch.
    let before_lp = u64::from_le_bytes(
        data[STABLE_FEE_TIERS_0_FEES_START..STABLE_FEE_TIERS_0_FEES_START + 8]
            .try_into()
            .unwrap(),
    );
    let before_protocol = u64::from_le_bytes(
        data[STABLE_FEE_TIERS_0_FEES_START + 8..STABLE_FEE_TIERS_0_FEES_START + 16]
            .try_into()
            .unwrap(),
    );
    let before_creator = u64::from_le_bytes(
        data[STABLE_FEE_TIERS_0_FEES_START + 16..STABLE_FEE_TIERS_0_FEES_START + 24]
            .try_into()
            .unwrap(),
    );
    println!(
        "stable_fee_tiers[0].fees BEFORE patch = {{lp:{before_lp}, protocol:{before_protocol}, creator:{before_creator}}} \
         (expect {{0, 95, 30}}, matching fee_tiers[0] per fees-06#2)"
    );
    assert_eq!(
        (before_lp, before_protocol, before_creator),
        (0, 95, 30),
        "offset math is wrong — aborting before sending any misleading transactions"
    );

    data[STABLE_FEE_TIERS_0_FEES_START..STABLE_FEE_TIERS_0_FEES_START + 8]
        .copy_from_slice(&11u64.to_le_bytes());
    data[STABLE_FEE_TIERS_0_FEES_START + 8..STABLE_FEE_TIERS_0_FEES_START + 16]
        .copy_from_slice(&22u64.to_le_bytes());
    data[STABLE_FEE_TIERS_0_FEES_START + 16..STABLE_FEE_TIERS_0_FEES_START + 24]
        .copy_from_slice(&33u64.to_le_bytes());
    data[FLAT_FEES_START..FLAT_FEES_START + 8].copy_from_slice(&44u64.to_le_bytes());
    data[FLAT_FEES_START + 8..FLAT_FEES_START + 16].copy_from_slice(&55u64.to_le_bytes());
    data[FLAT_FEES_START + 16..FLAT_FEES_START + 24].copy_from_slice(&66u64.to_le_bytes());
    println!(
        "PATCHED: stable_fee_tiers[0].fees={{11,22,33}}  flat_fees={{44,55,66}}  \
         fee_tiers[0].fees left at real {{0,95,30}}\n"
    );

    svm.set_account(
        fee_config,
        Account {
            lamports: 37_290_293,
            data,
            owner: program_id,
            executable: false,
            rent_epoch: 0,
        },
    )
    .expect("seed patched fee_config account");

    let payer = Keypair::new();
    svm.airdrop(&payer.pubkey(), 10_000_000_000)
        .expect("airdrop payer");

    for &is_pump_pool in &[false, true] {
        for &is_new_quote_mint in &[false, true] {
            let ix = build_get_fees_ix(
                program_id,
                fee_config,
                config_program_id,
                is_pump_pool,
                1_000_000,
                is_new_quote_mint,
            );
            let blockhash = svm.latest_blockhash();
            let msg = Message::new(&[ix], Some(&payer.pubkey()));
            let tx = Transaction::new(&[&payer], msg, blockhash);
            print!("is_pump_pool={is_pump_pool} is_new_quote_mint={is_new_quote_mint} -> ");
            match svm.send_transaction(tx) {
                Ok(meta) => {
                    let d = &meta.return_data.data;
                    if d.len() == 24 {
                        let lp = u64::from_le_bytes(d[0..8].try_into().unwrap());
                        let protocol = u64::from_le_bytes(d[8..16].try_into().unwrap());
                        let creator = u64::from_le_bytes(d[16..24].try_into().unwrap());
                        let which = match (lp, protocol, creator) {
                            (0, 95, 30) => "fee_tiers",
                            (11, 22, 33) => "stable_fee_tiers",
                            (44, 55, 66) => "flat_fees",
                            _ => "UNRECOGNIZED",
                        };
                        println!("Fees{{lp:{lp}, protocol:{protocol}, creator:{creator}}} => read from {which}");
                    } else {
                        println!("unexpected return_data len {}: {:?}", d.len(), d);
                    }
                }
                Err(e) => println!("FAILED: {e:?}"),
            }
        }
    }
}
