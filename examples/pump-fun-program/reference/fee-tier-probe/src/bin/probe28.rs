//! Compute-unit baseline from the real, deployed `pump.so`/`pump_fees.so` for
//! `buy`/`sell`, at the same trade size (15_000_000_000_000 base units) used
//! by `pump-bonding-curve/programs/pump/tests/pump_test.rs`'s
//! `buy_purchases_tokens_and_updates_reserves`/`sell_returns_tokens_and_updates_reserves`
//! — lets `naclac profile`'s numbers for our reimplementation be compared
//! directly against the real program's own cost. Account wiring for both
//! instructions is copied from the already-empirically-confirmed `probe22`
//! (buy) / `probe20` (sell) rather than re-derived — see those files for how
//! each account list/discriminator was settled.

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

const BUY_DISCRIMINATOR: [u8; 8] = [102, 6, 61, 18, 1, 218, 235, 234];
const SELL_DISCRIMINATOR: [u8; 8] = [51, 230, 133, 164, 1, 127, 131, 173];
const GLOBAL_DISCRIMINATOR: [u8; 8] = [167, 232, 232, 177, 200, 108, 114, 127];
const BONDING_CURVE_DISCRIMINATOR: [u8; 8] = [23, 183, 248, 55, 96, 216, 172, 96];
const BUYBACK_VAULT_DISCRIMINATOR: [u8; 8] = [153, 166, 71, 144, 179, 189, 137, 251];

const TRADE_AMOUNT: u64 = 15_000_000_000_000;
const VIRTUAL_TOKEN_RESERVES: u64 = 1_073_000_000_000_000;
const VIRTUAL_SOL_RESERVES: u64 = 30_000_000_000;
const REAL_TOKEN_RESERVES: u64 = 793_100_000_000_000;
const TOKEN_TOTAL_SUPPLY: u64 = 1_000_000_000_000_000;

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
    data.push(1); // initialized
    data.extend_from_slice(&[0u8; 32]); // authority
    data.extend_from_slice(fee_recipient.as_ref()); // fee_recipient
    data.extend_from_slice(&[0u8; 8 * 5]); // initial_virtual_token_reserves..fee_basis_points
    data.extend_from_slice(&[0u8; 32]); // withdraw_authority
    data.push(0); // enable_migrate
    data.extend_from_slice(&[0u8; 8 * 2]); // pool_migration_fee, creator_fee_basis_points
    data.extend_from_slice(&[0u8; 32 * 7]); // fee_recipients[7]
    data.extend_from_slice(&[0u8; 32 * 2]); // set_creator_authority, admin_set_creator_authority
    data.push(0); // create_v2_enabled
    data.extend_from_slice(&[0u8; 32]); // whitelist_pda
    data.extend_from_slice(&[0u8; 32]); // reserved_fee_recipient
    data.push(0); // mayhem_mode_enabled
    data.extend_from_slice(&[0u8; 32 * 7]); // reserved_fee_recipients[7]
    data.push(0); // is_cashback_enabled
    for _ in 0..8 {
        data.extend_from_slice(buyback_fee_recipient.as_ref()); // buyback_fee_recipients[8]
    }
    data.extend_from_slice(&0u64.to_le_bytes()); // buyback_basis_points
    data.extend_from_slice(&0u64.to_le_bytes()); // initial_virtual_quote_reserves
    data.extend_from_slice(&[0u8; 32]); // whitelisted_quote_mints[1]
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

/// Loads a fresh `svm` with `pump.so`/`pump_fees.so`, a funded `fee_recipient`,
/// a registered `buyback_fee_recipient` (index 0), a real `FeeConfig`, and a
/// bonding curve at pump-fun's real default reserves — everything both `buy`
/// and `sell` need in common. Returns the pieces each instruction's account
/// list still has to reference individually.
#[allow(clippy::type_complexity)]
fn setup(
    real_sol_reserves: u64,
) -> (LiteSVM, Pubkey, Pubkey, Pubkey, Pubkey, Pubkey, Keypair, Pubkey, Pubkey, Pubkey, Pubkey, Pubkey) {
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

    let (bonding_curve, _) = Pubkey::find_program_address(&[b"bonding-curve", mint.as_ref()], &pump_program);
    svm.set_account(
        bonding_curve,
        Account {
            lamports: 10_000_000 + real_sol_reserves,
            data: bonding_curve_account_data(VIRTUAL_TOKEN_RESERVES, VIRTUAL_SOL_RESERVES, REAL_TOKEN_RESERVES, real_sol_reserves, TOKEN_TOTAL_SUPPLY, &creator),
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

    let (creator_vault, _) = Pubkey::find_program_address(&[b"creator-vault", creator.as_ref()], &pump_program);
    svm.set_account(creator_vault, Account { lamports: 10_000_000, data: vec![], owner: system_program, executable: false, rent_epoch: 0 }).unwrap();

    let (event_authority, _) = Pubkey::find_program_address(&[b"__event_authority"], &pump_program);
    let (bonding_curve_v2, _) = Pubkey::find_program_address(&[b"bonding-curve-v2", mint.as_ref()], &pump_program);

    let user = Keypair::new();
    svm.airdrop(&user.pubkey(), 100_000_000_000).unwrap();

    (svm, pump_program, pump_fees_program, token_program, system_program, mint, user, bonding_curve, associated_bonding_curve, creator_vault, event_authority, bonding_curve_v2)
}

fn run_buy() -> u64 {
    let (mut svm, pump_program, pump_fees_program, token_program, system_program, mint, user, bonding_curve, associated_bonding_curve, creator_vault, event_authority, bonding_curve_v2) =
        setup(0);

    let (global, _) = Pubkey::find_program_address(&[b"global"], &pump_program);
    let (fee_config, _) = Pubkey::find_program_address(&[b"fee_config", pump_program.as_ref()], &pump_fees_program);
    let (buyback_fee_recipient, _) = Pubkey::find_program_address(&[b"buyback-vault", &[0u8]], &pump_fees_program);
    let global_account = svm.get_account(&global).unwrap();
    let fee_recipient = Pubkey::try_from(&global_account.data[41..73]).unwrap();

    let associated_user = ata_address(&user.pubkey(), &mint, &token_program);
    svm.set_account(
        associated_user,
        Account { lamports: 2_039_280, data: token_account_data(&mint, &user.pubkey(), 0), owner: token_program, executable: false, rent_epoch: 0 },
    )
    .unwrap();

    let (global_volume_accumulator, _) = Pubkey::find_program_address(&[b"global_volume_accumulator"], &pump_program);
    svm.set_account(
        global_volume_accumulator,
        Account { lamports: 10_000_000, data: vec![202, 42, 246, 43, 142, 190, 30, 255].into_iter().chain([0u8; 8 * 3 + 32 + 8 * 30 + 8 * 30]).collect(), owner: pump_program, executable: false, rent_epoch: 0 },
    )
    .unwrap();

    let (user_volume_accumulator, _) = Pubkey::find_program_address(&[b"user_volume_accumulator", user.pubkey().as_ref()], &pump_program);
    svm.set_account(
        user_volume_accumulator,
        Account {
            lamports: 10_000_000,
            data: vec![86, 255, 112, 14, 102, 53, 154, 250].into_iter().chain(user.pubkey().to_bytes()).chain([0u8; 1 + 8 + 8 + 8 + 8 + 1 + 8 + 8 + 8 + 8 + 31]).collect(),
            owner: pump_program,
            executable: false,
            rent_epoch: 0,
        },
    )
    .unwrap();

    let mut data = BUY_DISCRIMINATOR.to_vec();
    data.extend_from_slice(&TRADE_AMOUNT.to_le_bytes());
    data.extend_from_slice(&1_000_000_000u64.to_le_bytes()); // max_sol_cost
    data.push(0); // track_volume = None
    data.extend_from_slice(&[0u8; 3]);

    let ix = Instruction {
        program_id: pump_program,
        accounts: vec![
            AccountMeta::new_readonly(global, false),
            AccountMeta::new(fee_recipient, false),
            AccountMeta::new_readonly(mint, false),
            AccountMeta::new(bonding_curve, false),
            AccountMeta::new(associated_bonding_curve, false),
            AccountMeta::new(associated_user, false),
            AccountMeta::new(user.pubkey(), true),
            AccountMeta::new_readonly(system_program, false),
            AccountMeta::new_readonly(token_program, false),
            AccountMeta::new(creator_vault, false),
            AccountMeta::new_readonly(event_authority, false),
            AccountMeta::new_readonly(pump_program, false),
            AccountMeta::new(global_volume_accumulator, false),
            AccountMeta::new(user_volume_accumulator, false),
            AccountMeta::new_readonly(fee_config, false),
            AccountMeta::new_readonly(pump_fees_program, false),
            AccountMeta::new_readonly(bonding_curve_v2, false),
            AccountMeta::new(buyback_fee_recipient, false),
        ],
        data,
    };

    let budget_ix = set_compute_unit_limit_ix(400_000);
    let blockhash = svm.latest_blockhash();
    let msg = Message::new(&[budget_ix, ix], Some(&user.pubkey()));
    let tx = Transaction::new(&[&user], msg, blockhash);

    match svm.send_transaction(tx) {
        Ok(meta) => {
            println!("buy: SUCCESS, {} CU", meta.compute_units_consumed);
            meta.compute_units_consumed
        }
        Err(e) => {
            println!("buy: FAILED: {:?}", e.err);
            for line in &e.meta.logs {
                println!("  LOG: {line}");
            }
            0
        }
    }
}

fn run_sell() -> u64 {
    // Give the curve some already-bought-in SOL, matching realistic sell
    // conditions (same as `probe20`).
    const REAL_SOL_RESERVES: u64 = 5_000_000_000;
    let (mut svm, pump_program, pump_fees_program, token_program, system_program, mint, user, bonding_curve, associated_bonding_curve, creator_vault, event_authority, bonding_curve_v2) =
        setup(REAL_SOL_RESERVES);

    let (global, _) = Pubkey::find_program_address(&[b"global"], &pump_program);
    let (fee_config, _) = Pubkey::find_program_address(&[b"fee_config", pump_program.as_ref()], &pump_fees_program);
    let (buyback_fee_recipient, _) = Pubkey::find_program_address(&[b"buyback-vault", &[0u8]], &pump_fees_program);
    let global_account = svm.get_account(&global).unwrap();
    let fee_recipient = Pubkey::try_from(&global_account.data[41..73]).unwrap();

    let associated_user = ata_address(&user.pubkey(), &mint, &token_program);
    svm.set_account(
        associated_user,
        Account { lamports: 2_039_280, data: token_account_data(&mint, &user.pubkey(), TRADE_AMOUNT), owner: token_program, executable: false, rent_epoch: 0 },
    )
    .unwrap();

    let mut data = SELL_DISCRIMINATOR.to_vec();
    data.extend_from_slice(&TRADE_AMOUNT.to_le_bytes());
    data.extend_from_slice(&0u64.to_le_bytes()); // min_sol_output = 0, take whatever

    let ix = Instruction {
        program_id: pump_program,
        accounts: vec![
            AccountMeta::new_readonly(global, false),
            AccountMeta::new(fee_recipient, false),
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
            AccountMeta::new_readonly(bonding_curve_v2, false),
            AccountMeta::new(buyback_fee_recipient, false),
        ],
        data,
    };

    let budget_ix = set_compute_unit_limit_ix(400_000);
    let blockhash = svm.latest_blockhash();
    let msg = Message::new(&[budget_ix, ix], Some(&user.pubkey()));
    let tx = Transaction::new(&[&user], msg, blockhash);

    match svm.send_transaction(tx) {
        Ok(meta) => {
            println!("sell: SUCCESS, {} CU", meta.compute_units_consumed);
            meta.compute_units_consumed
        }
        Err(e) => {
            println!("sell: FAILED: {:?}", e.err);
            for line in &e.meta.logs {
                println!("  LOG: {line}");
            }
            0
        }
    }
}

fn main() {
    println!("Real pump.so/pump_fees.so CU baseline at TRADE_AMOUNT = {TRADE_AMOUNT}\n");
    let buy_cu = run_buy();
    let sell_cu = run_sell();
    println!("\n--- SUMMARY ---");
    println!("buy:  {buy_cu} CU");
    println!("sell: {sell_cu} CU");
}
