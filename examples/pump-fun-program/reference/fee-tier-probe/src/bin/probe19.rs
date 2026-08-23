//! Resolves the remaining unverified assumptions in our own `buy.rs` (see
//! `docs/plan/fees-07-donation-relay-progress.md`'s "new assumptions" list),
//! against the real `pump.so` + `pump_fees.so`. Each case builds its own
//! fresh mint/bonding-curve/accounts (fully isolated — a failure in one
//! case doesn't affect the others) and runs a single `buy`.
//!
//! Case A (x2): reads `pump_fees::get_fees`'s CPI instruction data directly
//! out of `TransactionMetadata::inner_instructions` — this is the exact
//! `market_cap_lamports` (u128) `pump.so` computed internally before the
//! CPI, not inferred from output. Two different reserve configs give two
//! data points to solve the formula.
//!
//! Case B: passes a `fee_recipient` account that does NOT match
//! `global.fee_recipient`, to see whether the real program validates it.
//!
//! Case C: a larger trade (to make rounding differences visible) with
//! `global.buyback_basis_points` set nonzero, decoding the full real
//! `TradeEvent` (fee/creator_fee/buyback_fee/buyback_fee_basis_points) to
//! check the fee-total rounding rule and the buyback-split formula.

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
const GET_FEES_DISCRIMINATOR: [u8; 8] = [231, 37, 126, 85, 207, 91, 63, 52];
const GLOBAL_DISCRIMINATOR: [u8; 8] = [167, 232, 232, 177, 200, 108, 114, 127];
const BONDING_CURVE_DISCRIMINATOR: [u8; 8] = [23, 183, 248, 55, 96, 216, 172, 96];
const GLOBAL_VOLUME_ACCUMULATOR_DISCRIMINATOR: [u8; 8] = [202, 42, 246, 43, 142, 190, 30, 255];
const USER_VOLUME_ACCUMULATOR_DISCRIMINATOR: [u8; 8] = [86, 255, 112, 14, 102, 53, 154, 250];
const BUYBACK_VAULT_DISCRIMINATOR: [u8; 8] = [153, 166, 71, 144, 179, 189, 137, 251];
const TRADE_EVENT_DISCRIMINATOR: [u8; 8] = [0xbd, 0xdb, 0x7f, 0xd3, 0x4e, 0xe6, 0x61, 0xee];

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

fn global_account_data(fee_recipient: &Pubkey, buyback_fee_recipient: &Pubkey, buyback_basis_points: u64) -> Vec<u8> {
    let mut data = Vec::new();
    data.extend_from_slice(&GLOBAL_DISCRIMINATOR);
    data.push(1); // initialized
    data.extend_from_slice(&[0u8; 32]); // authority
    data.extend_from_slice(fee_recipient.as_ref());
    data.extend_from_slice(&0u64.to_le_bytes()); // initial_virtual_token_reserves
    data.extend_from_slice(&0u64.to_le_bytes()); // initial_virtual_sol_reserves
    data.extend_from_slice(&0u64.to_le_bytes()); // initial_real_token_reserves
    data.extend_from_slice(&0u64.to_le_bytes()); // token_total_supply
    data.extend_from_slice(&0u64.to_le_bytes()); // fee_basis_points
    data.extend_from_slice(&[0u8; 32]); // withdraw_authority
    data.push(0); // enable_migrate
    data.extend_from_slice(&0u64.to_le_bytes()); // pool_migration_fee
    data.extend_from_slice(&0u64.to_le_bytes()); // creator_fee_basis_points
    data.extend_from_slice(&[0u8; 32 * 7]); // fee_recipients[7]
    data.extend_from_slice(&[0u8; 32]); // set_creator_authority
    data.extend_from_slice(&[0u8; 32]); // admin_set_creator_authority
    data.push(0); // create_v2_enabled
    data.extend_from_slice(&[0u8; 32]); // whitelist_pda
    data.extend_from_slice(&[0u8; 32]); // reserved_fee_recipient
    data.push(0); // mayhem_mode_enabled
    data.extend_from_slice(&[0u8; 32 * 7]); // reserved_fee_recipients[7]
    data.push(0); // is_cashback_enabled
    for _ in 0..8 {
        data.extend_from_slice(buyback_fee_recipient.as_ref());
    }
    data.extend_from_slice(&buyback_basis_points.to_le_bytes());
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
    data.extend_from_slice(&[0u8; 32]); // quote_mint = native SOL
    data
}

// Layout (absolute offsets, matching the real Borsh `FeeConfig`): disc[0..8),
// bump[8], admin[9..41), flat_fees{lp[41..49), protocol[49..57), creator[57..65)},
// fee_tiers: Vec<FeeTier> len[65..69) then entries of
// {threshold:u128[16] + fees{lp[8],protocol[8],creator[8]}} = 40 bytes each,
// first entry's lp_fee_bps at [85..93). Confirmed against real
// `pump_fees.json`'s `FeeConfig`/`FeeTier` field order.
fn fee_config_account_data(bump: u8, lp_fee_bps_override: Option<u64>) -> Vec<u8> {
    let mut data = include_bytes!("../../fixtures/fee_config.bin").to_vec();
    data[8] = bump;
    if let Some(lp) = lp_fee_bps_override {
        data[85..93].copy_from_slice(&lp.to_le_bytes());
    }
    data
}

fn dump_fee_config_tier0_sanity_check() {
    let data = include_bytes!("../../fixtures/fee_config.bin");
    let tier0_threshold = u128::from_le_bytes(data[69..85].try_into().unwrap());
    let tier0_lp = u64::from_le_bytes(data[85..93].try_into().unwrap());
    let tier0_protocol = u64::from_le_bytes(data[93..101].try_into().unwrap());
    let tier0_creator = u64::from_le_bytes(data[101..109].try_into().unwrap());
    println!(
        "fee_config.bin tier[0] sanity check: threshold={tier0_threshold} lp={tier0_lp} protocol={tier0_protocol} creator={tier0_creator}"
    );
    println!("  (expect protocol=95, creator=30 to match Case A1/A2/B's observed fee_basis_points/creator_fee_basis_points at market_cap=0)\n");
}

fn global_volume_accumulator_account_data() -> Vec<u8> {
    let mut data = Vec::new();
    data.extend_from_slice(&GLOBAL_VOLUME_ACCUMULATOR_DISCRIMINATOR);
    data.extend_from_slice(&[0u8; 8 * 3 + 32 + 8 * 30 + 8 * 30]);
    data
}

fn user_volume_accumulator_account_data(user: &Pubkey) -> Vec<u8> {
    let mut data = Vec::new();
    data.extend_from_slice(&USER_VOLUME_ACCUMULATOR_DISCRIMINATOR);
    data.extend_from_slice(user.as_ref());
    data.extend_from_slice(&[0u8; 1 + 8 + 8 + 8 + 8 + 1 + 8 + 8 + 8 + 8 + 31]);
    data
}

fn buyback_vault_account_data(authority: &Pubkey) -> Vec<u8> {
    let mut data = Vec::new();
    data.extend_from_slice(&BUYBACK_VAULT_DISCRIMINATOR);
    data.extend_from_slice(authority.as_ref());
    data.extend_from_slice(&[0u8; 8 + 8 + 8 + 8 + 8 + 128]);
    data
}

/// Decodes the real `TradeEvent` (full field list, confirmed against
/// `pump-public-docs/idl/pump.json`). Assumes `shareholders` is empty — true
/// for every fixture in this probe (no `SharingConfig` set up).
#[derive(Debug)]
#[allow(dead_code)]
struct TradeEvent {
    sol_amount: u64,
    token_amount: u64,
    is_buy: bool,
    virtual_sol_reserves: u64,
    virtual_token_reserves: u64,
    real_sol_reserves: u64,
    real_token_reserves: u64,
    fee_basis_points: u64,
    fee: u64,
    creator_fee_basis_points: u64,
    creator_fee: u64,
    track_volume: bool,
    ix_name: String,
    mayhem_mode: bool,
    cashback_fee_basis_points: u64,
    cashback: u64,
    buyback_fee_basis_points: u64,
    buyback_fee: u64,
}

fn decode_trade_event(raw: &[u8]) -> Option<TradeEvent> {
    if raw.len() < 8 || raw[0..8] != TRADE_EVENT_DISCRIMINATOR {
        return None;
    }
    let d = &raw[8..];
    let rd_u64 = |o: usize| u64::from_le_bytes(d[o..o + 8].try_into().unwrap());
    let ix_name_len = u32::from_le_bytes(d[250..254].try_into().unwrap()) as usize;
    let ix_name = String::from_utf8_lossy(&d[254..254 + ix_name_len]).to_string();
    let mut o = 254 + ix_name_len;
    let mayhem_mode = d[o] != 0;
    o += 1;
    let cashback_fee_basis_points = rd_u64(o);
    o += 8;
    let cashback = rd_u64(o);
    o += 8;
    let buyback_fee_basis_points = rd_u64(o);
    o += 8;
    let buyback_fee = rd_u64(o);
    Some(TradeEvent {
        sol_amount: rd_u64(32),
        token_amount: rd_u64(40),
        is_buy: d[48] != 0,
        virtual_sol_reserves: rd_u64(89),
        virtual_token_reserves: rd_u64(97),
        real_sol_reserves: rd_u64(105),
        real_token_reserves: rd_u64(113),
        fee_basis_points: rd_u64(153),
        fee: rd_u64(161),
        creator_fee_basis_points: rd_u64(201),
        creator_fee: rd_u64(209),
        track_volume: d[217] != 0,
        ix_name,
        mayhem_mode,
        cashback_fee_basis_points,
        cashback,
        buyback_fee_basis_points,
        buyback_fee,
    })
}

struct CaseConfig<'a> {
    label: &'a str,
    virtual_token_reserves: u64,
    virtual_sol_reserves: u64,
    real_token_reserves: u64,
    token_total_supply: u64,
    buy_amount: u64,
    buyback_basis_points: u64,
    /// If true, the `fee_recipient` account passed to the ix does NOT match
    /// `global.fee_recipient` — tests whether the real program validates it.
    mismatch_fee_recipient: bool,
    real_sol_reserves: u64,
    lp_fee_bps_override: Option<u64>,
}

fn run_case(cfg: &CaseConfig) {
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

    let real_fee_recipient = Keypair::new();
    svm.airdrop(&real_fee_recipient.pubkey(), 10_000_000_000).unwrap();
    let wrong_fee_recipient = Keypair::new();
    svm.airdrop(&wrong_fee_recipient.pubkey(), 10_000_000_000).unwrap();
    let fee_recipient_used = if cfg.mismatch_fee_recipient { wrong_fee_recipient.pubkey() } else { real_fee_recipient.pubkey() };

    let (buyback_fee_recipient, _) = Pubkey::find_program_address(&[b"buyback-vault", &[0u8]], &pump_fees_program);
    svm.set_account(
        buyback_fee_recipient,
        Account { lamports: 10_000_000, data: buyback_vault_account_data(&Pubkey::new_unique()), owner: pump_fees_program, executable: false, rent_epoch: 0 },
    )
    .unwrap();

    let (global, _) = Pubkey::find_program_address(&[b"global"], &pump_program);
    svm.set_account(
        global,
        Account {
            lamports: 10_000_000,
            data: global_account_data(&real_fee_recipient.pubkey(), &buyback_fee_recipient, cfg.buyback_basis_points),
            owner: pump_program,
            executable: false,
            rent_epoch: 0,
        },
    )
    .unwrap();

    let (fee_config, fc_bump) = Pubkey::find_program_address(&[b"fee_config", pump_program.as_ref()], &pump_fees_program);
    svm.set_account(
        fee_config,
        Account { lamports: 10_000_000, data: fee_config_account_data(fc_bump, cfg.lp_fee_bps_override), owner: pump_fees_program, executable: false, rent_epoch: 0 },
    )
    .unwrap();

    let (bonding_curve, _) = Pubkey::find_program_address(&[b"bonding-curve", mint.as_ref()], &pump_program);
    svm.set_account(
        bonding_curve,
        Account {
            lamports: 10_000_000 + cfg.real_sol_reserves,
            data: bonding_curve_account_data(
                cfg.virtual_token_reserves,
                cfg.virtual_sol_reserves,
                cfg.real_token_reserves,
                cfg.real_sol_reserves,
                cfg.token_total_supply,
                &creator,
            ),
            owner: pump_program,
            executable: false,
            rent_epoch: 0,
        },
    )
    .unwrap();

    let associated_bonding_curve = ata_address(&bonding_curve, &mint, &token_program);
    svm.set_account(
        associated_bonding_curve,
        Account { lamports: 2_039_280, data: token_account_data(&mint, &bonding_curve, cfg.token_total_supply), owner: token_program, executable: false, rent_epoch: 0 },
    )
    .unwrap();

    let user = Keypair::new();
    svm.airdrop(&user.pubkey(), 100_000_000_000).unwrap();
    let associated_user = ata_address(&user.pubkey(), &mint, &token_program);
    svm.set_account(
        associated_user,
        Account { lamports: 2_039_280, data: token_account_data(&mint, &user.pubkey(), 0), owner: token_program, executable: false, rent_epoch: 0 },
    )
    .unwrap();

    let (creator_vault, _) = Pubkey::find_program_address(&[b"creator-vault", creator.as_ref()], &pump_program);
    svm.set_account(creator_vault, Account { lamports: 10_000_000, data: vec![], owner: system_program, executable: false, rent_epoch: 0 }).unwrap();
    let (event_authority, _) = Pubkey::find_program_address(&[b"__event_authority"], &pump_program);

    let (global_volume_accumulator, _) = Pubkey::find_program_address(&[b"global_volume_accumulator"], &pump_program);
    svm.set_account(
        global_volume_accumulator,
        Account { lamports: 10_000_000, data: global_volume_accumulator_account_data(), owner: pump_program, executable: false, rent_epoch: 0 },
    )
    .unwrap();

    let (user_volume_accumulator, _) = Pubkey::find_program_address(&[b"user_volume_accumulator", user.pubkey().as_ref()], &pump_program);
    svm.set_account(
        user_volume_accumulator,
        Account { lamports: 10_000_000, data: user_volume_accumulator_account_data(&user.pubkey()), owner: pump_program, executable: false, rent_epoch: 0 },
    )
    .unwrap();

    let mut data = BUY_DISCRIMINATOR.to_vec();
    data.extend_from_slice(&cfg.buy_amount.to_le_bytes());
    data.extend_from_slice(&100_000_000_000u64.to_le_bytes());
    data.push(1); // track_volume = true
    data.extend_from_slice(&[0u8; 3]);

    let ix = Instruction {
        program_id: pump_program,
        accounts: vec![
            AccountMeta::new_readonly(global, false),
            AccountMeta::new(fee_recipient_used, false),
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
            AccountMeta::new_readonly(Pubkey::find_program_address(&[b"bonding-curve-v2", mint.as_ref()], &pump_program).0, false),
            AccountMeta::new(buyback_fee_recipient, false),
        ],
        data,
    };

    println!("\n=== {} ===", cfg.label);
    let budget_ix = set_compute_unit_limit_ix(400_000);
    let blockhash = svm.latest_blockhash();
    let msg = Message::new(&[budget_ix, ix], Some(&user.pubkey()));
    let tx = Transaction::new(&[&user], msg, blockhash);

    match svm.send_transaction(tx) {
        Ok(meta) => {
            println!("RESULT: SUCCESS");

            // Case A: pull get_fees's exact CPI instruction data straight
            // out of the instruction trace — the real market_cap_lamports
            // pump.so computed, not inferred.
            for inner_list in &meta.inner_instructions {
                for inner in inner_list {
                    let ix_data = &inner.instruction.data;
                    if ix_data.len() >= 8 && ix_data[0..8] == GET_FEES_DISCRIMINATOR {
                        let is_pump_pool = ix_data[8];
                        let market_cap_lamports = u128::from_le_bytes(ix_data[9..25].try_into().unwrap());
                        let trade_size_lamports = u64::from_le_bytes(ix_data[25..33].try_into().unwrap());
                        let is_new_quote_mint = ix_data[33];
                        println!(
                            "  get_fees CPI args: is_pump_pool={is_pump_pool} market_cap_lamports={market_cap_lamports} trade_size_lamports={trade_size_lamports} is_new_quote_mint={is_new_quote_mint}"
                        );
                    }
                }
            }

            for line in &meta.logs {
                if let Some(b64) = line.strip_prefix("Program data: ") {
                    if let Ok(raw) = base64_decode(b64) {
                        if let Some(ev) = decode_trade_event(&raw) {
                            println!("  TradeEvent: {ev:#?}");
                        }
                    }
                }
            }
        }
        Err(e) => {
            println!("RESULT: FAILED: {:?}", e.err);
            for line in &e.meta.logs {
                println!("  LOG: {line}");
            }
        }
    }
}

// Minimal base64 decoder (avoids adding a new crate dependency for one call site).
fn base64_decode(input: &str) -> Result<Vec<u8>, ()> {
    const TABLE: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut rev = [255u8; 256];
    for (i, &c) in TABLE.iter().enumerate() {
        rev[c as usize] = i as u8;
    }
    let clean: Vec<u8> = input.bytes().filter(|&b| b != b'=').collect();
    let mut out = Vec::with_capacity(clean.len() * 3 / 4);
    for chunk in clean.chunks(4) {
        let vals: Vec<u8> = chunk.iter().map(|&b| rev[b as usize]).collect();
        if vals.iter().any(|&v| v == 255) {
            return Err(());
        }
        out.push((vals[0] << 2) | (vals.get(1).copied().unwrap_or(0) >> 4));
        if vals.len() > 2 {
            out.push((vals[1] << 4) | (vals[2] >> 2));
        }
        if vals.len() > 3 {
            out.push((vals[2] << 6) | vals[3]);
        }
    }
    Ok(out)
}

fn main() {
    dump_fee_config_tier0_sanity_check();

    // Case A: two different reserve configs, same tiny trade size, to solve
    // the market_cap_lamports formula from two real data points.
    run_case(&CaseConfig {
        label: "Case A1: market_cap formula, config 1",
        virtual_token_reserves: 1_073_000_000_000_000,
        virtual_sol_reserves: 30_000_000_000,
        real_token_reserves: 793_100_000_000_000,
        token_total_supply: 1_000_000_000_000_000,
        buy_amount: 500_000,
        buyback_basis_points: 0,
        mismatch_fee_recipient: false,
        real_sol_reserves: 0,
        lp_fee_bps_override: None,
    });
    run_case(&CaseConfig {
        label: "Case A2: market_cap formula, config 2 (different reserves)",
        virtual_token_reserves: 800_000_000_000_000,
        virtual_sol_reserves: 60_000_000_000,
        real_token_reserves: 600_000_000_000_000,
        token_total_supply: 1_000_000_000_000_000,
        buy_amount: 500_000,
        buyback_basis_points: 0,
        mismatch_fee_recipient: false,
        real_sol_reserves: 0,
        lp_fee_bps_override: None,
    });

    // Case B: fee_recipient account passed does NOT match global.fee_recipient.
    run_case(&CaseConfig {
        label: "Case B: mismatched fee_recipient",
        virtual_token_reserves: 1_073_000_000_000_000,
        virtual_sol_reserves: 30_000_000_000,
        real_token_reserves: 793_100_000_000_000,
        token_total_supply: 1_000_000_000_000_000,
        buy_amount: 500_000,
        buyback_basis_points: 0,
        mismatch_fee_recipient: true,
        real_sol_reserves: 0,
        lp_fee_bps_override: None,
    });

    // Case C: much larger trade (visible rounding), buyback_basis_points
    // nonzero (2000 = 20%, arbitrary but distinguishable), to check the
    // fee-total rounding rule and the buyback-share formula.
    run_case(&CaseConfig {
        label: "Case C: large trade + nonzero buyback_basis_points",
        virtual_token_reserves: 1_073_000_000_000_000,
        virtual_sol_reserves: 30_000_000_000,
        real_token_reserves: 793_100_000_000_000,
        token_total_supply: 1_000_000_000_000_000,
        buy_amount: 50_000_000_000_000, // much bigger — 50 trillion base units
        buyback_basis_points: 2_000,
        mismatch_fee_recipient: false,
        real_sol_reserves: 0,
        lp_fee_bps_override: None,
    });

    // Case D: nonzero PRE-TRADE real_sol_reserves (5 SOL) — A1/A2 both had
    // real_sol_reserves=0 and both returned market_cap_lamports=0, so this
    // tests whether market_cap is derived from real_sol_reserves specifically.
    run_case(&CaseConfig {
        label: "Case D: nonzero pre-trade real_sol_reserves (5 SOL)",
        virtual_token_reserves: 1_073_000_000_000_000,
        virtual_sol_reserves: 30_000_000_000,
        real_token_reserves: 793_100_000_000_000,
        token_total_supply: 1_000_000_000_000_000,
        buy_amount: 500_000,
        buyback_basis_points: 0,
        mismatch_fee_recipient: false,
        real_sol_reserves: 5_000_000_000,
        lp_fee_bps_override: None,
    });

    // Case E: same as A1, but fee_config's tier[0].lp_fee_bps patched to a
    // nonzero, distinguishable value (500 = 5%) — A1/A2/C all had lp=0 in
    // the real dumped fixture, so this tests whether/how a nonzero
    // lp_fee_bps factors into the actual SOL deducted from the buyer.
    run_case(&CaseConfig {
        label: "Case E: nonzero lp_fee_bps (500) in fee_config tier[0]",
        virtual_token_reserves: 1_073_000_000_000_000,
        virtual_sol_reserves: 30_000_000_000,
        real_token_reserves: 793_100_000_000_000,
        token_total_supply: 1_000_000_000_000_000,
        buy_amount: 500_000,
        buyback_basis_points: 0,
        mismatch_fee_recipient: false,
        real_sol_reserves: 0,
        lp_fee_bps_override: Some(500),
    });
}
