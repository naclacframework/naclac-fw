//! Resolves `sell`'s exact curve formula against real `pump.so`. Hand-deriving
//! the constant-product math for `sell` (tokens ADDED to virtual_token_reserves,
//! unlike `buy` where they're removed) gives `sol_out = tokens *
//! virtual_sol_reserves / (virtual_token_reserves + tokens)` — a PLUS sign —
//! which contradicts an old progress-doc note claiming a MINUS-sign formula
//! (`+ 1`) is "relevant for sell". This settles it directly: real
//! `virtual_sol_reserves`/`real_sol_reserves` deltas after a real `sell`,
//! compared against both candidate formulas.
//!
//! Real classic `sell`: discriminator [51,230,133,164,1,127,131,173], args
//! `amount: u64, min_sol_output: u64`. Real 14-account order: global,
//! fee_recipient, mint, bonding_curve, associated_bonding_curve,
//! associated_user, user, system_program, creator_vault, token_program,
//! event_authority, program, fee_config, fee_program — then (real client
//! SDK, `pump_legacy.rs::sell_instruction`) always appends bonding_curve_v2
//! (readonly), then buyback_fee_recipient.

use litesvm::LiteSVM;
use solana_sdk::{
    account::Account,
    clock::Clock,
    instruction::{AccountMeta, Instruction},
    message::Message,
    pubkey::Pubkey,
    signature::{Keypair, Signer},
    transaction::Transaction,
};
use std::str::FromStr;

const PUMP_PROGRAM_ID: &str = "6EF8rrecthR5Dkzon8Nwu78hRvfCKubJ14M5uBEwF6P";
const PUMP_FEES_PROGRAM_ID: &str = "pfeeUxB6jkeY1Hxd7CsFCAjcbHA9rWtchMGdZ6VojVZ";
const TOKEN_PROGRAM_ID: &str = "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA";
const ASSOCIATED_TOKEN_PROGRAM_ID: &str = "ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL";
const SYSTEM_PROGRAM_ID: &str = "11111111111111111111111111111111";

const SELL_DISCRIMINATOR: [u8; 8] = [51, 230, 133, 164, 1, 127, 131, 173];
const GLOBAL_DISCRIMINATOR: [u8; 8] = [167, 232, 232, 177, 200, 108, 114, 127];
const BONDING_CURVE_DISCRIMINATOR: [u8; 8] = [23, 183, 248, 55, 96, 216, 172, 96];
const BUYBACK_VAULT_DISCRIMINATOR: [u8; 8] = [153, 166, 71, 144, 179, 189, 137, 251];

fn pk(s: &str) -> Pubkey {
    Pubkey::from_str(s).unwrap()
}

fn set_compute_unit_limit_ix(units: u32) -> Instruction {
    let mut data = vec![2u8];
    data.extend_from_slice(&units.to_le_bytes());
    Instruction { program_id: pk("ComputeBudget111111111111111111111111111111"), accounts: vec![], data }
}

fn ata_address(owner: &Pubkey, mint: &Pubkey, token_program: &Pubkey) -> Pubkey {
    Pubkey::find_program_address(&[owner.as_ref(), token_program.as_ref(), mint.as_ref()], &pk(ASSOCIATED_TOKEN_PROGRAM_ID)).0
}

fn mint_account_data(decimals: u8) -> Vec<u8> {
    let mut data = vec![0u8; 82];
    data[44] = decimals;
    data[45] = 1;
    data
}

fn token_account_data(mint: &Pubkey, owner: &Pubkey, amount: u64) -> Vec<u8> {
    let mut data = vec![0u8; 165];
    data[0..32].copy_from_slice(mint.as_ref());
    data[32..64].copy_from_slice(owner.as_ref());
    data[64..72].copy_from_slice(&amount.to_le_bytes());
    data[108] = 1;
    data
}

fn global_account_data(fee_recipient: &Pubkey, buyback_fee_recipient: &Pubkey) -> Vec<u8> {
    let mut data = Vec::new();
    data.extend_from_slice(&GLOBAL_DISCRIMINATOR);
    data.push(1);
    data.extend_from_slice(&[0u8; 32]);
    data.extend_from_slice(fee_recipient.as_ref());
    data.extend_from_slice(&[0u8; 8 * 5]);
    data.extend_from_slice(&[0u8; 32]);
    data.push(0);
    data.extend_from_slice(&[0u8; 8 * 2]);
    data.extend_from_slice(&[0u8; 32 * 7]);
    data.extend_from_slice(&[0u8; 32 * 2]);
    data.push(0);
    data.extend_from_slice(&[0u8; 32]);
    data.extend_from_slice(&[0u8; 32]);
    data.push(0);
    data.extend_from_slice(&[0u8; 32 * 7]);
    data.push(0);
    for _ in 0..8 {
        data.extend_from_slice(buyback_fee_recipient.as_ref());
    }
    data.extend_from_slice(&0u64.to_le_bytes());
    data.extend_from_slice(&0u64.to_le_bytes());
    data.extend_from_slice(&[0u8; 32]);
    data
}

fn bonding_curve_account_data(
    virtual_token_reserves: u64,
    virtual_sol_reserves: u64,
    real_token_reserves: u64,
    real_sol_reserves: u64,
    token_total_supply: u64,
    creator: &Pubkey,
) -> Vec<u8> {
    let mut data = Vec::new();
    data.extend_from_slice(&BONDING_CURVE_DISCRIMINATOR);
    data.extend_from_slice(&virtual_token_reserves.to_le_bytes());
    data.extend_from_slice(&virtual_sol_reserves.to_le_bytes());
    data.extend_from_slice(&real_token_reserves.to_le_bytes());
    data.extend_from_slice(&real_sol_reserves.to_le_bytes());
    data.extend_from_slice(&token_total_supply.to_le_bytes());
    data.push(0); // complete
    data.extend_from_slice(creator.as_ref());
    data.push(0); // is_mayhem_mode
    data.push(0); // is_cashback_coin
    data.extend_from_slice(&[0u8; 32]); // quote_mint
    data
}

fn decode_bonding_curve(data: &[u8]) -> (u64, u64, u64, u64) {
    let d = &data[8..];
    let rd_u64 = |o: usize| u64::from_le_bytes(d[o..o + 8].try_into().unwrap());
    (rd_u64(0), rd_u64(8), rd_u64(16), rd_u64(24))
}

fn fee_config_account_data(bump: u8) -> Vec<u8> {
    let mut data = include_bytes!("../../fixtures/fee_config.bin").to_vec();
    data[8] = bump;
    data
}

fn buyback_vault_account_data(authority: &Pubkey) -> Vec<u8> {
    let mut data = Vec::new();
    data.extend_from_slice(&BUYBACK_VAULT_DISCRIMINATOR);
    data.extend_from_slice(authority.as_ref());
    data.extend_from_slice(&[0u8; 8 + 8 + 8 + 8 + 8 + 128]);
    data
}

fn main() {
    let pump_program = pk(PUMP_PROGRAM_ID);
    let pump_fees_program = pk(PUMP_FEES_PROGRAM_ID);
    let token_program = pk(TOKEN_PROGRAM_ID);
    let system_program = pk(SYSTEM_PROGRAM_ID);

    let mut svm = LiteSVM::new();
    for (program_id, so_name) in [(pump_program, "pump.so"), (pump_fees_program, "pump_fees.so")] {
        let so_path = format!("{}/../pump-rust-client/artifacts/{}", env!("CARGO_MANIFEST_DIR"), so_name);
        svm.add_program_from_file(program_id, &so_path).unwrap_or_else(|e| panic!("load {so_name}: {e:?}"));
    }
    svm.set_sysvar::<Clock>(&Clock { unix_timestamp: 1_700_000_000, slot: 100, epoch: 0, leader_schedule_epoch: 0, epoch_start_timestamp: 0 });

    let creator = Pubkey::new_unique();
    let mint = Pubkey::new_unique();
    svm.set_account(mint, Account { lamports: 10_000_000, data: mint_account_data(6), owner: token_program, executable: false, rent_epoch: 0 }).unwrap();

    let fee_recipient = Keypair::new();
    svm.airdrop(&fee_recipient.pubkey(), 10_000_000_000).unwrap();

    let (buyback_fee_recipient, _) = Pubkey::find_program_address(&[b"buyback-vault", &[0u8]], &pump_fees_program);
    svm.set_account(
        buyback_fee_recipient,
        Account { lamports: 10_000_000, data: buyback_vault_account_data(&Pubkey::new_unique()), owner: pump_fees_program, executable: false, rent_epoch: 0 },
    )
    .unwrap();

    let (global, _) = Pubkey::find_program_address(&[b"global"], &pump_program);
    svm.set_account(
        global,
        Account { lamports: 10_000_000, data: global_account_data(&fee_recipient.pubkey(), &buyback_fee_recipient), owner: pump_program, executable: false, rent_epoch: 0 },
    )
    .unwrap();

    let (fee_config, fc_bump) = Pubkey::find_program_address(&[b"fee_config", pump_program.as_ref()], &pump_fees_program);
    svm.set_account(
        fee_config,
        Account { lamports: 10_000_000, data: fee_config_account_data(fc_bump), owner: pump_fees_program, executable: false, rent_epoch: 0 },
    )
    .unwrap();

    // A curve that's already seen some buys (nonzero real_sol_reserves),
    // matching realistic sell conditions.
    const VIRTUAL_TOKEN_RESERVES: u64 = 1_073_000_000_000_000;
    const VIRTUAL_SOL_RESERVES: u64 = 35_000_000_000; // 30 SOL base + 5 SOL already bought in
    const REAL_TOKEN_RESERVES: u64 = 793_000_000_000_000; // some already sold out
    const REAL_SOL_RESERVES: u64 = 5_000_000_000;
    const TOKEN_TOTAL_SUPPLY: u64 = 1_000_000_000_000_000;

    let (bonding_curve, _) = Pubkey::find_program_address(&[b"bonding-curve", mint.as_ref()], &pump_program);
    svm.set_account(
        bonding_curve,
        Account {
            lamports: 10_000_000 + REAL_SOL_RESERVES,
            data: bonding_curve_account_data(VIRTUAL_TOKEN_RESERVES, VIRTUAL_SOL_RESERVES, REAL_TOKEN_RESERVES, REAL_SOL_RESERVES, TOKEN_TOTAL_SUPPLY, &creator),
            owner: pump_program,
            executable: false,
            rent_epoch: 0,
        },
    )
    .unwrap();

    let associated_bonding_curve = ata_address(&bonding_curve, &mint, &token_program);
    svm.set_account(
        associated_bonding_curve,
        Account { lamports: 2_039_280, data: token_account_data(&mint, &bonding_curve, TOKEN_TOTAL_SUPPLY - REAL_TOKEN_RESERVES), owner: token_program, executable: false, rent_epoch: 0 },
    )
    .unwrap();

    let user = Keypair::new();
    svm.airdrop(&user.pubkey(), 100_000_000_000).unwrap();
    // Large enough relative to virtual_token_reserves (~1.073e15) that a
    // +/- sign difference in the denominator actually produces distinguishable
    // outputs (500_000 was ~700,000x too small to tell the two apart), but
    // smaller than the first attempt (200 trillion), which tripped an
    // unrelated real `Overflow` guard — same order of magnitude as `buy`'s
    // own successful large-trade probe (`probe19` Case C used 50 trillion).
    const SELL_AMOUNT: u64 = 50_000_000_000_000;
    let associated_user = ata_address(&user.pubkey(), &mint, &token_program);
    svm.set_account(
        associated_user,
        Account { lamports: 2_039_280, data: token_account_data(&mint, &user.pubkey(), SELL_AMOUNT), owner: token_program, executable: false, rent_epoch: 0 },
    )
    .unwrap();

    let (creator_vault, _) = Pubkey::find_program_address(&[b"creator-vault", creator.as_ref()], &pump_program);
    svm.set_account(creator_vault, Account { lamports: 10_000_000, data: vec![], owner: system_program, executable: false, rent_epoch: 0 }).unwrap();
    let (event_authority, _) = Pubkey::find_program_address(&[b"__event_authority"], &pump_program);

    let mut data = SELL_DISCRIMINATOR.to_vec();
    data.extend_from_slice(&SELL_AMOUNT.to_le_bytes());
    data.extend_from_slice(&0u64.to_le_bytes()); // min_sol_output = 0, take whatever

    let ix = Instruction {
        program_id: pump_program,
        accounts: vec![
            AccountMeta::new_readonly(global, false),
            AccountMeta::new(fee_recipient.pubkey(), false),
            AccountMeta::new_readonly(mint, false),
            AccountMeta::new(bonding_curve, false),
            AccountMeta::new(associated_bonding_curve, false),
            AccountMeta::new(associated_user, false),
            AccountMeta::new(user.pubkey(), true),
            AccountMeta::new_readonly(system_program, false),
            AccountMeta::new(creator_vault, false),
            AccountMeta::new_readonly(token_program, false),
            AccountMeta::new_readonly(event_authority, false),
            AccountMeta::new_readonly(pump_program, false),
            AccountMeta::new_readonly(fee_config, false),
            AccountMeta::new_readonly(pump_fees_program, false),
            AccountMeta::new_readonly(Pubkey::find_program_address(&[b"bonding-curve-v2", mint.as_ref()], &pump_program).0, false),
            AccountMeta::new(buyback_fee_recipient, false),
        ],
        data,
    };

    println!("--- BEFORE ---");
    println!("virtual_token_reserves = {VIRTUAL_TOKEN_RESERVES}");
    println!("virtual_sol_reserves   = {VIRTUAL_SOL_RESERVES}");
    println!("sell amount (tokens)   = {SELL_AMOUNT}");

    let minus_formula = (SELL_AMOUNT as u128 * VIRTUAL_SOL_RESERVES as u128) / (VIRTUAL_TOKEN_RESERVES as u128 - SELL_AMOUNT as u128);
    let plus_formula = (SELL_AMOUNT as u128 * VIRTUAL_SOL_RESERVES as u128) / (VIRTUAL_TOKEN_RESERVES as u128 + SELL_AMOUNT as u128);
    println!("candidate (minus-sign, old doc note): {minus_formula}");
    println!("candidate (plus-sign, hand-derived):  {plus_formula}\n");

    let budget_ix = set_compute_unit_limit_ix(400_000);
    let blockhash = svm.latest_blockhash();
    let msg = Message::new(&[budget_ix, ix], Some(&user.pubkey()));
    let tx = Transaction::new(&[&user], msg, blockhash);

    match svm.send_transaction(tx) {
        Ok(meta) => {
            println!("RESULT: SUCCESS");
            for line in &meta.logs {
                println!("  LOG: {line}");
            }
            let bc = svm.get_account(&bonding_curve).unwrap();
            let (vtr, vsr, rtr, rsr) = decode_bonding_curve(&bc.data);
            println!("\n--- AFTER ---");
            println!("virtual_token_reserves = {vtr} (delta {})", vtr as i64 - VIRTUAL_TOKEN_RESERVES as i64);
            println!("virtual_sol_reserves   = {vsr} (delta {})", vsr as i64 - VIRTUAL_SOL_RESERVES as i64);
            println!("real_token_reserves    = {rtr} (delta {})", rtr as i64 - REAL_TOKEN_RESERVES as i64);
            println!("real_sol_reserves      = {rsr} (delta {})", rsr as i64 - REAL_SOL_RESERVES as i64);
            let sol_delta = REAL_SOL_RESERVES as i64 - rsr as i64;
            println!("\nActual gross SOL moved out of curve: {sol_delta}");
            println!("Matches minus-formula ({minus_formula})? {}", sol_delta as u128 == minus_formula || (sol_delta as u128).abs_diff(minus_formula) <= 2);
            println!("Matches plus-formula ({plus_formula})? {}", sol_delta as u128 == plus_formula || (sol_delta as u128).abs_diff(plus_formula) <= 2);
        }
        Err(e) => {
            println!("RESULT: FAILED: {:?}", e.err);
            for line in &e.meta.logs {
                println!("  LOG: {line}");
            }
        }
    }
}
