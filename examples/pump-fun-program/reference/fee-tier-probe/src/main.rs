//! One-shot empirical probe for pump_fees::get_fees, run entirely in-process
//! via litesvm against the REAL compiled pump_fees.so and a REAL cloned
//! fee_config account (fetched live from mainnet). No network, no other
//! programs, no bonding curve setup needed — get_fees takes its selection
//! inputs (is_pump_pool, market_cap_lamports, trade_size_lamports,
//! is_new_quote_mint) directly as instruction args, so we can just call it
//! directly and diff the returned `Fees` across inputs.
//!
//! Goal: determine whether `is_new_quote_mint` (or market_cap_lamports
//! magnitude) changes which fee tier table (`fee_tiers` vs
//! `stable_fee_tiers`) get_fees reads from. See
//! examples/pump-fun-program/docs/plan/fees-06-open-questions.md #2.

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

/// `get_fees` discriminator, from pump_fees.json's IDL.
const GET_FEES_DISCRIMINATOR: [u8; 8] = [231, 37, 126, 85, 207, 91, 63, 52];

/// Manually Borsh-encode get_fees's args. `OptionBool` is, per the IDL's
/// `types` section, a tuple struct wrapping a single `bool` field (NOT an
/// actual `Option<bool>` — no presence/absence tag), so it's just 1 byte.
fn build_get_fees_ix(
    program_id: Pubkey,
    fee_config: Pubkey,
    config_program_id: Pubkey,
    is_pump_pool: bool,
    market_cap_lamports: u128,
    trade_size_lamports: u64,
    is_new_quote_mint: bool,
) -> Instruction {
    let mut data = Vec::with_capacity(8 + 1 + 16 + 8 + 1);
    data.extend_from_slice(&GET_FEES_DISCRIMINATOR);
    data.push(is_pump_pool as u8);
    data.extend_from_slice(&market_cap_lamports.to_le_bytes());
    data.extend_from_slice(&trade_size_lamports.to_le_bytes());
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

fn run_get_fees(
    svm: &mut LiteSVM,
    payer: &Keypair,
    program_id: Pubkey,
    fee_config: Pubkey,
    config_program_id: Pubkey,
    is_pump_pool: bool,
    market_cap_lamports: u128,
    is_new_quote_mint: bool,
) {
    let ix = build_get_fees_ix(
        program_id,
        fee_config,
        config_program_id,
        is_pump_pool,
        market_cap_lamports,
        0,
        is_new_quote_mint,
    );
    let blockhash = svm.latest_blockhash();
    let msg = Message::new(&[ix], Some(&payer.pubkey()));
    let tx = Transaction::new(&[payer], msg, blockhash);

    println!(
        "--- is_pump_pool={is_pump_pool} market_cap_lamports={market_cap_lamports} is_new_quote_mint={is_new_quote_mint} ---"
    );
    match svm.send_transaction(tx) {
        Ok(meta) => {
            println!("SUCCESS");
            println!("logs:");
            for line in meta.logs.iter() {
                println!("  {line}");
            }
            println!("return_data (raw bytes): {:?}", meta.return_data.data);
            if meta.return_data.data.len() == 24 {
                let lp = u64::from_le_bytes(meta.return_data.data[0..8].try_into().unwrap());
                let protocol =
                    u64::from_le_bytes(meta.return_data.data[8..16].try_into().unwrap());
                let creator =
                    u64::from_le_bytes(meta.return_data.data[16..24].try_into().unwrap());
                println!(
                    "decoded Fees {{ lp_fee_bps: {lp}, protocol_fee_bps: {protocol}, creator_fee_bps: {creator} }}"
                );
            }
        }
        Err(e) => {
            println!("FAILED: {e:?}");
        }
    }
}

fn main() {
    let program_id = Pubkey::from_str(PUMP_FEES_PROGRAM_ID).unwrap();
    let config_program_id = Pubkey::from_str(PUMP_PROGRAM_ID).unwrap();
    let fee_config = Pubkey::from_str(FEE_CONFIG).unwrap();

    let mut svm = LiteSVM::new();

    let so_path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../pump-rust-client/artifacts/pump_fees.so"
    );
    svm.add_program_from_file(program_id, so_path)
        .expect("load pump_fees.so");

    let fee_config_bytes = include_bytes!("../fixtures/fee_config.bin");
    svm.set_account(
        fee_config,
        Account {
            lamports: 37_290_293,
            data: fee_config_bytes.to_vec(),
            owner: program_id,
            executable: false,
            rent_epoch: 0,
        },
    )
    .expect("seed fee_config account");

    let payer = Keypair::new();
    svm.airdrop(&payer.pubkey(), 10_000_000_000)
        .expect("airdrop payer");

    // Representative market caps spanning small -> very large, to surface
    // any tier-boundary or table-selection differences.
    let market_caps: [u128; 6] = [
        1_000_000,
        1_000_000_000,
        50_000_000_000,
        400_000_000_000,
        2_000_000_000_000,
        50_000_000_000_000,
    ];

    for &mc in market_caps.iter() {
        for &is_pump_pool in &[false, true] {
            run_get_fees(
                &mut svm,
                &payer,
                program_id,
                fee_config,
                config_program_id,
                is_pump_pool,
                mc,
                false,
            );
            run_get_fees(
                &mut svm,
                &payer,
                program_id,
                fee_config,
                config_program_id,
                is_pump_pool,
                mc,
                true,
            );
        }
    }
}
