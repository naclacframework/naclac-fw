//! Probes the real `pump.so`'s `buy` (CPI-ing into the real `pump_fees.so`
//! for fee config) to empirically determine the graduation trigger: does
//! `bonding_curve.complete` flip to `true` when `real_token_reserves` would
//! be depleted by a buy, and is the trade capped at the available balance or
//! does it reject outright? Needed to correctly implement `buy` in our own
//! `pump-bonding-curve` reimplementation (scoped: classic `buy` only).
//!
//! Strategy: set up a bonding curve with a deliberately small
//! `real_token_reserves`, buy an amount that exceeds it, and inspect the
//! resulting `bonding_curve.complete` flag and reserve values.

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

const PUMP_PROGRAM_ID: &str = "6EF8rrecthR5Dkzon8Nwu78hRvfCKubJ14M5uBEwF6P";
const PUMP_FEES_PROGRAM_ID: &str = "pfeeUxB6jkeY1Hxd7CsFCAjcbHA9rWtchMGdZ6VojVZ";
const TOKEN_PROGRAM_ID: &str = "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA";
const ASSOCIATED_TOKEN_PROGRAM_ID: &str = "ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL";
const SYSTEM_PROGRAM_ID: &str = "11111111111111111111111111111111";

const BUY_DISCRIMINATOR: [u8; 8] = [102, 6, 61, 18, 1, 218, 235, 234];
const GLOBAL_DISCRIMINATOR: [u8; 8] = [167, 232, 232, 177, 200, 108, 114, 127];
const BONDING_CURVE_DISCRIMINATOR: [u8; 8] = [23, 183, 248, 55, 96, 216, 172, 96];
const FEE_CONFIG_DISCRIMINATOR: [u8; 8] = [143, 52, 146, 187, 219, 123, 76, 155];
const GLOBAL_VOLUME_ACCUMULATOR_DISCRIMINATOR: [u8; 8] = [202, 42, 246, 43, 142, 190, 30, 255];
const USER_VOLUME_ACCUMULATOR_DISCRIMINATOR: [u8; 8] = [86, 255, 112, 14, 102, 53, 154, 250];

fn pk(s: &str) -> Pubkey {
    Pubkey::from_str(s).unwrap()
}

fn set_compute_unit_limit_ix(units: u32) -> Instruction {
    let mut data = vec![2u8];
    data.extend_from_slice(&units.to_le_bytes());
    Instruction {
        program_id: pk("ComputeBudget111111111111111111111111111111"),
        accounts: vec![],
        data,
    }
}

fn ata_address(owner: &Pubkey, mint: &Pubkey, token_program: &Pubkey) -> Pubkey {
    Pubkey::find_program_address(
        &[owner.as_ref(), token_program.as_ref(), mint.as_ref()],
        &pk(ASSOCIATED_TOKEN_PROGRAM_ID),
    )
    .0
}

fn mint_account_data(mint_authority: Option<&Pubkey>, decimals: u8) -> Vec<u8> {
    let mut data = vec![0u8; 82];
    if let Some(auth) = mint_authority {
        data[0..4].copy_from_slice(&1u32.to_le_bytes());
        data[4..36].copy_from_slice(auth.as_ref());
    }
    data[36..44].copy_from_slice(&0u64.to_le_bytes());
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

// initialized, authority, fee_recipient, initial_virtual_token_reserves,
// initial_virtual_sol_reserves, initial_real_token_reserves,
// token_total_supply, fee_basis_points, withdraw_authority, enable_migrate,
// pool_migration_fee, creator_fee_basis_points, fee_recipients[7],
// set_creator_authority, admin_set_creator_authority, create_v2_enabled,
// whitelist_pda, reserved_fee_recipient, mayhem_mode_enabled,
// reserved_fee_recipients[7], is_cashback_enabled, buyback_fee_recipients[8],
// buyback_basis_points, initial_virtual_quote_reserves,
// whitelisted_quote_mints[1] — confirmed field order from
// pump-public-docs/idl/pump.json.
fn global_account_data(fee_recipient: &Pubkey, buyback_fee_recipient: &Pubkey) -> Vec<u8> {
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
    // All 8 entries set to the SAME pubkey — guarantees a match regardless
    // of which index the real program's (unresolved) selection logic
    // computes, sidestepping the need to know that algorithm for this probe.
    for _ in 0..8 {
        data.extend_from_slice(buyback_fee_recipient.as_ref());
    }
    data.extend_from_slice(&0u64.to_le_bytes()); // buyback_basis_points
    data.extend_from_slice(&0u64.to_le_bytes()); // initial_virtual_quote_reserves
    data.extend_from_slice(&[0u8; 32]); // whitelisted_quote_mints[1]
    data
}

// virtual_token_reserves, virtual_quote_reserves, real_token_reserves,
// real_quote_reserves, token_total_supply, complete, creator,
// is_mayhem_mode, is_cashback_coin, quote_mint — confirmed field order from
// pump-public-docs/idl/pump.json.
struct BondingCurveFixture {
    virtual_token_reserves: u64,
    virtual_sol_reserves: u64,
    real_token_reserves: u64,
    real_sol_reserves: u64,
    token_total_supply: u64,
    complete: bool,
    creator: Pubkey,
    quote_mint: Pubkey,
}

fn bonding_curve_account_data(f: &BondingCurveFixture) -> Vec<u8> {
    let mut data = Vec::new();
    data.extend_from_slice(&BONDING_CURVE_DISCRIMINATOR);
    data.extend_from_slice(&f.virtual_token_reserves.to_le_bytes());
    data.extend_from_slice(&f.virtual_sol_reserves.to_le_bytes());
    data.extend_from_slice(&f.real_token_reserves.to_le_bytes());
    data.extend_from_slice(&f.real_sol_reserves.to_le_bytes());
    data.extend_from_slice(&f.token_total_supply.to_le_bytes());
    data.push(f.complete as u8);
    data.extend_from_slice(f.creator.as_ref());
    data.push(0); // is_mayhem_mode
    data.push(0); // is_cashback_coin
    data.extend_from_slice(f.quote_mint.as_ref());
    data
}

fn fee_config_account_data(bump: u8, admin: &Pubkey) -> Vec<u8> {
    let mut data = Vec::new();
    data.extend_from_slice(&FEE_CONFIG_DISCRIMINATOR);
    data.push(bump);
    data.extend_from_slice(admin.as_ref());
    data.extend_from_slice(&0u64.to_le_bytes()); // flat_fees.lp_fee_bps
    data.extend_from_slice(&0u64.to_le_bytes()); // flat_fees.protocol_fee_bps
    data.extend_from_slice(&0u64.to_le_bytes()); // flat_fees.creator_fee_bps
    data.extend_from_slice(&0u32.to_le_bytes()); // fee_tiers: empty Vec
    data.extend_from_slice(&0u32.to_le_bytes()); // stable_fee_tiers: empty Vec
    data
}

// start_time, end_time, seconds_in_a_day, mint, total_token_supply[u64;30],
// sol_volumes[u64;30] — confirmed field order from pump-public-docs/idl/pump.json.
fn global_volume_accumulator_account_data() -> Vec<u8> {
    let mut data = Vec::new();
    data.extend_from_slice(&GLOBAL_VOLUME_ACCUMULATOR_DISCRIMINATOR);
    data.extend_from_slice(&0i64.to_le_bytes()); // start_time
    data.extend_from_slice(&0i64.to_le_bytes()); // end_time
    data.extend_from_slice(&0i64.to_le_bytes()); // seconds_in_a_day
    data.extend_from_slice(&[0u8; 32]); // mint
    data.extend_from_slice(&[0u8; 8 * 30]); // total_token_supply[30]
    data.extend_from_slice(&[0u8; 8 * 30]); // sol_volumes[30]
    data
}

// user, needs_claim, total_unclaimed_tokens, total_claimed_tokens,
// current_sol_volume, last_update_timestamp, has_total_claimed_tokens,
// cashback_earned, total_cashback_claimed, stable_cashback_earned,
// total_stable_cashback_claimed — confirmed field order from
// pump-public-docs/idl/pump.json.
fn user_volume_accumulator_account_data(user: &Pubkey) -> Vec<u8> {
    let mut data = Vec::new();
    data.extend_from_slice(&USER_VOLUME_ACCUMULATOR_DISCRIMINATOR);
    data.extend_from_slice(user.as_ref());
    data.push(0); // needs_claim
    data.extend_from_slice(&0u64.to_le_bytes()); // total_unclaimed_tokens
    data.extend_from_slice(&0u64.to_le_bytes()); // total_claimed_tokens
    data.extend_from_slice(&0u64.to_le_bytes()); // current_sol_volume
    data.extend_from_slice(&0i64.to_le_bytes()); // last_update_timestamp
    data.push(0); // has_total_claimed_tokens
    data.extend_from_slice(&0u64.to_le_bytes()); // cashback_earned
    data.extend_from_slice(&0u64.to_le_bytes()); // total_cashback_claimed
    data.extend_from_slice(&0u64.to_le_bytes()); // stable_cashback_earned
    data.extend_from_slice(&0u64.to_le_bytes()); // total_stable_cashback_claimed
    // Real deployed account needs 137 bytes total (129 struct + 8 disc); the
    // IDL's listed fields above only account for 98 — probe15 confirmed a
    // 31-byte gap empirically (ConstraintSpace: Left=137, Right=106 with the
    // fields above alone). Likely an undocumented trailing reserved blob.
    data.extend_from_slice(&[0u8; 31]);
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
        svm.add_program_from_file(program_id, &so_path)
            .unwrap_or_else(|e| panic!("load {so_name}: {e:?}"));
    }

    let creator = Pubkey::new_unique();
    let mint = Pubkey::new_unique();
    svm.set_account(
        mint,
        Account {
            lamports: 10_000_000,
            data: mint_account_data(None, 6),
            owner: token_program,
            executable: false,
            rent_epoch: 0,
        },
    )
    .unwrap();

    let fee_recipient = Keypair::new();
    let buyback_fee_recipient = Pubkey::new_unique();
    let (global, _global_bump) = Pubkey::find_program_address(&[b"global"], &pump_program);
    svm.set_account(
        global,
        Account {
            lamports: 10_000_000,
            data: global_account_data(&fee_recipient.pubkey(), &buyback_fee_recipient),
            owner: pump_program,
            executable: false,
            rent_epoch: 0,
        },
    )
    .unwrap();

    let (fee_config, fc_bump) = Pubkey::find_program_address(&[b"fee_config", pump_program.as_ref()], &pump_fees_program);
    svm.set_account(
        fee_config,
        Account {
            lamports: 10_000_000,
            data: fee_config_account_data(fc_bump, &Pubkey::new_unique()),
            owner: pump_fees_program,
            executable: false,
            rent_epoch: 0,
        },
    )
    .unwrap();

    // Deliberately small real_token_reserves — probing what happens when a
    // buy's requested amount exceeds it.
    const VIRTUAL_TOKEN_RESERVES: u64 = 1_073_000_000_000_000;
    const VIRTUAL_SOL_RESERVES: u64 = 30_000_000_000;
    const REAL_TOKEN_RESERVES: u64 = 500_000; // small on purpose
    const REAL_SOL_RESERVES: u64 = 0;
    const TOKEN_TOTAL_SUPPLY: u64 = 1_000_000_000_000_000;

    let (bonding_curve, _bc_bump) = Pubkey::find_program_address(&[b"bonding-curve", mint.as_ref()], &pump_program);
    svm.set_account(
        bonding_curve,
        Account {
            lamports: 10_000_000,
            data: bonding_curve_account_data(&BondingCurveFixture {
                virtual_token_reserves: VIRTUAL_TOKEN_RESERVES,
                virtual_sol_reserves: VIRTUAL_SOL_RESERVES,
                real_token_reserves: REAL_TOKEN_RESERVES,
                real_sol_reserves: REAL_SOL_RESERVES,
                token_total_supply: TOKEN_TOTAL_SUPPLY,
                complete: false,
                creator,
                quote_mint: Pubkey::default(),
            }),
            owner: pump_program,
            executable: false,
            rent_epoch: 0,
        },
    )
    .unwrap();

    let associated_bonding_curve = ata_address(&bonding_curve, &mint, &token_program);
    svm.set_account(
        associated_bonding_curve,
        Account {
            lamports: 2_039_280,
            data: token_account_data(&mint, &bonding_curve, REAL_TOKEN_RESERVES),
            owner: token_program,
            executable: false,
            rent_epoch: 0,
        },
    )
    .unwrap();

    let user = Keypair::new();
    svm.airdrop(&user.pubkey(), 100_000_000_000).unwrap();

    let associated_user = ata_address(&user.pubkey(), &mint, &token_program);
    svm.set_account(
        associated_user,
        Account {
            lamports: 2_039_280,
            data: token_account_data(&mint, &user.pubkey(), 0),
            owner: token_program,
            executable: false,
            rent_epoch: 0,
        },
    )
    .unwrap();

    let (creator_vault, _cv_bump) = Pubkey::find_program_address(&[b"creator-vault", creator.as_ref()], &pump_program);
    let (event_authority, _) = Pubkey::find_program_address(&[b"__event_authority"], &pump_program);

    let (global_volume_accumulator, _) = Pubkey::find_program_address(&[b"global_volume_accumulator"], &pump_program);
    svm.set_account(
        global_volume_accumulator,
        Account {
            lamports: 10_000_000,
            data: global_volume_accumulator_account_data(),
            owner: pump_program,
            executable: false,
            rent_epoch: 0,
        },
    )
    .unwrap();

    let (user_volume_accumulator, _) = Pubkey::find_program_address(&[b"user_volume_accumulator", user.pubkey().as_ref()], &pump_program);
    svm.set_account(
        user_volume_accumulator,
        Account {
            lamports: 10_000_000,
            data: user_volume_accumulator_account_data(&user.pubkey()),
            owner: pump_program,
            executable: false,
            rent_epoch: 0,
        },
    )
    .unwrap();

    println!("--- known pubkeys ---");
    println!("bonding_curve = {bonding_curve}");
    println!("REAL_TOKEN_RESERVES (before) = {REAL_TOKEN_RESERVES}");
    println!("---------------------");

    // Request an amount that exceeds the available real_token_reserves.
    const BUY_AMOUNT: u64 = 1_000_000;
    const MAX_SOL_COST: u64 = 100_000_000_000;

    let mut data = BUY_DISCRIMINATOR.to_vec();
    data.extend_from_slice(&BUY_AMOUNT.to_le_bytes());
    data.extend_from_slice(&MAX_SOL_COST.to_le_bytes());
    data.push(0); // track_volume: OptionBool = false
    // Real mainnet `buy` transactions have 28-byte instruction data (vs our
    // 25 without this) — 3 extra trailing bytes of unknown purpose (varies
    // between real samples, so likely not a fixed flag). Padding with zeros
    // to match the real total length and avoid a deserialization length
    // mismatch; not yet identified what these bytes represent.
    data.extend_from_slice(&[0u8; 3]);

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
            AccountMeta::new_readonly(token_program, false),
            AccountMeta::new(creator_vault, false),
            AccountMeta::new_readonly(event_authority, false),
            AccountMeta::new_readonly(pump_program, false),
            AccountMeta::new_readonly(global_volume_accumulator, false),
            AccountMeta::new(user_volume_accumulator, false),
            AccountMeta::new_readonly(fee_config, false),
            AccountMeta::new_readonly(pump_fees_program, false),
            // Remaining account: matches global.buyback_fee_recipients[0].
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
            println!("RESULT: SUCCESS");
            for line in &meta.logs {
                println!("  {line}");
            }
            let bc_account = svm.get_account(&bonding_curve).expect("bonding_curve must exist");
            let data = &bc_account.data;
            let virtual_token_reserves = u64::from_le_bytes(data[8..16].try_into().unwrap());
            let virtual_sol_reserves = u64::from_le_bytes(data[16..24].try_into().unwrap());
            let real_token_reserves = u64::from_le_bytes(data[24..32].try_into().unwrap());
            let real_sol_reserves = u64::from_le_bytes(data[32..40].try_into().unwrap());
            let complete = data[48];
            println!("\nAfter buy:");
            println!("  virtual_token_reserves = {virtual_token_reserves}");
            println!("  virtual_sol_reserves = {virtual_sol_reserves}");
            println!("  real_token_reserves = {real_token_reserves}");
            println!("  real_sol_reserves = {real_sol_reserves}");
            println!("  complete = {complete}");

            let user_ata = svm.get_account(&associated_user).expect("associated_user must exist");
            let user_balance = u64::from_le_bytes(user_ata.data[64..72].try_into().unwrap());
            println!("  user token balance = {user_balance} (requested {BUY_AMOUNT})");
        }
        Err(e) => {
            println!("RESULT: FAILED: {:?}", e.err);
            for line in &e.meta.logs {
                println!("  {line}");
            }
        }
    }
}
