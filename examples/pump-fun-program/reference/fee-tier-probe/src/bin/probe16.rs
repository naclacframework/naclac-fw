//! Probes the real `pump.so`'s `buy` to empirically determine how
//! `GlobalVolumeAccumulator`/`UserVolumeAccumulator` actually get updated
//! when `track_volume = true`. Real field lists for both are already
//! confirmed from `pump-public-docs/idl/pump.json` (see
//! `components/global_volume_accumulator.rs`/`user_volume_accumulator.rs`'s
//! own file-header comments) — what's NOT known is the update algorithm:
//! which fields change per buy, whether `sol_volumes`/`total_token_supply`
//! accumulate into a single "today" slot or spread across the 30-slot
//! array, and what happens across a day boundary (`seconds_in_a_day`).
//!
//! Strategy: one bonding curve, three sequential `buy`s with `track_volume =
//! true` — (1) baseline, (2) a second buy in the same "day", (3) a third buy
//! after jumping the Clock forward past `seconds_in_a_day` — snapshotting
//! both accumulator accounts' full real fields before/after each and
//! printing only what changed.

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
const GLOBAL_DISCRIMINATOR: [u8; 8] = [167, 232, 232, 177, 200, 108, 114, 127];
const BONDING_CURVE_DISCRIMINATOR: [u8; 8] = [23, 183, 248, 55, 96, 216, 172, 96];
const GLOBAL_VOLUME_ACCUMULATOR_DISCRIMINATOR: [u8; 8] = [202, 42, 246, 43, 142, 190, 30, 255];
const USER_VOLUME_ACCUMULATOR_DISCRIMINATOR: [u8; 8] = [86, 255, 112, 14, 102, 53, 154, 250];

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

fn global_account_data(fee_recipient: &Pubkey, buyback_fee_recipient: &Pubkey) -> Vec<u8> {
    let mut data = Vec::new();
    data.extend_from_slice(&GLOBAL_DISCRIMINATOR);
    data.push(1);
    data.extend_from_slice(&[0u8; 32]);
    data.extend_from_slice(fee_recipient.as_ref());
    data.extend_from_slice(&0u64.to_le_bytes());
    data.extend_from_slice(&0u64.to_le_bytes());
    data.extend_from_slice(&0u64.to_le_bytes());
    data.extend_from_slice(&0u64.to_le_bytes());
    data.extend_from_slice(&0u64.to_le_bytes());
    data.extend_from_slice(&[0u8; 32]);
    data.push(0);
    data.extend_from_slice(&0u64.to_le_bytes());
    data.extend_from_slice(&0u64.to_le_bytes());
    data.extend_from_slice(&[0u8; 32 * 7]);
    data.extend_from_slice(&[0u8; 32]);
    data.extend_from_slice(&[0u8; 32]);
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

struct BondingCurveFixture {
    virtual_token_reserves: u64,
    virtual_sol_reserves: u64,
    real_token_reserves: u64,
    real_sol_reserves: u64,
    token_total_supply: u64,
    complete: bool,
    creator: Pubkey,
    quote_mint: Pubkey,
    is_cashback_coin: bool,
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
    data.push(f.is_cashback_coin as u8);
    data.extend_from_slice(f.quote_mint.as_ref());
    data
}

// Use the REAL dumped `fee_config` bytes (`fixtures/fee_config.bin`, 4073
// bytes — already fetched earlier this project, real Borsh content,
// non-empty `fee_tiers`/`stable_fee_tiers`) instead of hand-reconstructing
// one. Only the `bump` byte (offset 8, right after the discriminator) is
// patched to match whatever bump WE derive for wherever we place this
// account, since Anchor's own `bump = fee_config.bump` constraint checks
// the stored value against a fresh re-derivation.
fn fee_config_account_data(bump: u8, _admin: &Pubkey) -> Vec<u8> {
    let mut data = include_bytes!("../../fixtures/fee_config.bin").to_vec();
    data[8] = bump;
    data
}

fn global_volume_accumulator_account_data() -> Vec<u8> {
    let mut data = Vec::new();
    data.extend_from_slice(&GLOBAL_VOLUME_ACCUMULATOR_DISCRIMINATOR);
    data.extend_from_slice(&0i64.to_le_bytes());
    data.extend_from_slice(&0i64.to_le_bytes());
    data.extend_from_slice(&0i64.to_le_bytes());
    data.extend_from_slice(&[0u8; 32]);
    data.extend_from_slice(&[0u8; 8 * 30]);
    data.extend_from_slice(&[0u8; 8 * 30]);
    data
}

const BUYBACK_VAULT_DISCRIMINATOR: [u8; 8] = [153, 166, 71, 144, 179, 189, 137, 251];

// Real field layout confirmed against `pump_fees.json`'s `types[].BuybackVault`
// (also matches our own `pump_fees` component byte-for-byte): authority(32) +
// total_claimed(8) + total_claimed_token1(8) + total_claimed_token2(8) +
// last_claimed(8) + claim_rate_limit(8) + _reserved(128) = 200, + 8-byte
// discriminator = 208 — matches the real account's `data_len=208` observed
// via `inspect_buy_tx`.
fn buyback_vault_account_data(authority: &Pubkey) -> Vec<u8> {
    let mut data = Vec::new();
    data.extend_from_slice(&BUYBACK_VAULT_DISCRIMINATOR);
    data.extend_from_slice(authority.as_ref());
    data.extend_from_slice(&0u64.to_le_bytes());
    data.extend_from_slice(&0u64.to_le_bytes());
    data.extend_from_slice(&0u64.to_le_bytes());
    data.extend_from_slice(&0i64.to_le_bytes());
    data.extend_from_slice(&0i64.to_le_bytes());
    data.extend_from_slice(&[0u8; 128]);
    data
}

fn user_volume_accumulator_account_data(user: &Pubkey) -> Vec<u8> {
    let mut data = Vec::new();
    data.extend_from_slice(&USER_VOLUME_ACCUMULATOR_DISCRIMINATOR);
    data.extend_from_slice(user.as_ref());
    data.push(0);
    data.extend_from_slice(&0u64.to_le_bytes());
    data.extend_from_slice(&0u64.to_le_bytes());
    data.extend_from_slice(&0u64.to_le_bytes());
    data.extend_from_slice(&0i64.to_le_bytes());
    data.push(0);
    data.extend_from_slice(&0u64.to_le_bytes());
    data.extend_from_slice(&0u64.to_le_bytes());
    data.extend_from_slice(&0u64.to_le_bytes());
    data.extend_from_slice(&0u64.to_le_bytes());
    data.extend_from_slice(&[0u8; 31]);
    data
}

#[derive(Debug, Clone, PartialEq)]
struct GlobalVolumeSnapshot {
    start_time: i64,
    end_time: i64,
    seconds_in_a_day: i64,
    mint: [u8; 32],
    total_token_supply: [u64; 30],
    sol_volumes: [u64; 30],
}

fn decode_global_volume(data: &[u8]) -> GlobalVolumeSnapshot {
    let d = &data[8..];
    let rd_i64 = |o: usize| i64::from_le_bytes(d[o..o + 8].try_into().unwrap());
    let rd_u64_arr = |o: usize| -> [u64; 30] {
        let mut out = [0u64; 30];
        for i in 0..30 {
            out[i] = u64::from_le_bytes(d[o + i * 8..o + i * 8 + 8].try_into().unwrap());
        }
        out
    };
    GlobalVolumeSnapshot {
        start_time: rd_i64(0),
        end_time: rd_i64(8),
        seconds_in_a_day: rd_i64(16),
        mint: d[24..56].try_into().unwrap(),
        total_token_supply: rd_u64_arr(56),
        sol_volumes: rd_u64_arr(296),
    }
}

fn print_global_volume_diff(label: &str, before: &GlobalVolumeSnapshot, after: &GlobalVolumeSnapshot) {
    println!("--- GlobalVolumeAccumulator diff ({label}) ---");
    if before.start_time != after.start_time {
        println!("  start_time: {} -> {}", before.start_time, after.start_time);
    }
    if before.end_time != after.end_time {
        println!("  end_time: {} -> {}", before.end_time, after.end_time);
    }
    if before.seconds_in_a_day != after.seconds_in_a_day {
        println!("  seconds_in_a_day: {} -> {}", before.seconds_in_a_day, after.seconds_in_a_day);
    }
    if before.mint != after.mint {
        println!("  mint: {:?} -> {:?}", before.mint, after.mint);
    }
    for i in 0..30 {
        if before.total_token_supply[i] != after.total_token_supply[i] {
            println!(
                "  total_token_supply[{i}]: {} -> {}",
                before.total_token_supply[i], after.total_token_supply[i]
            );
        }
        if before.sol_volumes[i] != after.sol_volumes[i] {
            println!("  sol_volumes[{i}]: {} -> {}", before.sol_volumes[i], after.sol_volumes[i]);
        }
    }
    if before == after {
        println!("  (no change)");
    }
}

#[derive(Debug, Clone, PartialEq)]
struct UserVolumeSnapshot {
    user: [u8; 32],
    needs_claim: u8,
    total_unclaimed_tokens: u64,
    total_claimed_tokens: u64,
    current_sol_volume: u64,
    last_update_timestamp: i64,
    has_total_claimed_tokens: u8,
    cashback_earned: u64,
    total_cashback_claimed: u64,
    stable_cashback_earned: u64,
    total_stable_cashback_claimed: u64,
    reserved_tail: [u8; 31],
}

fn decode_user_volume(data: &[u8]) -> UserVolumeSnapshot {
    let d = &data[8..];
    let rd_u64 = |o: usize| u64::from_le_bytes(d[o..o + 8].try_into().unwrap());
    let rd_i64 = |o: usize| i64::from_le_bytes(d[o..o + 8].try_into().unwrap());
    UserVolumeSnapshot {
        user: d[0..32].try_into().unwrap(),
        needs_claim: d[32],
        total_unclaimed_tokens: rd_u64(33),
        total_claimed_tokens: rd_u64(41),
        current_sol_volume: rd_u64(49),
        last_update_timestamp: rd_i64(57),
        has_total_claimed_tokens: d[65],
        cashback_earned: rd_u64(66),
        total_cashback_claimed: rd_u64(74),
        stable_cashback_earned: rd_u64(82),
        total_stable_cashback_claimed: rd_u64(90),
        reserved_tail: d[98..129].try_into().unwrap(),
    }
}

fn print_user_volume_diff(label: &str, before: &UserVolumeSnapshot, after: &UserVolumeSnapshot) {
    println!("--- UserVolumeAccumulator diff ({label}) ---");
    macro_rules! f {
        ($field:ident) => {
            if before.$field != after.$field {
                println!("  {}: {:?} -> {:?}", stringify!($field), before.$field, after.$field);
            }
        };
    }
    f!(user);
    f!(needs_claim);
    f!(total_unclaimed_tokens);
    f!(total_claimed_tokens);
    f!(current_sol_volume);
    f!(last_update_timestamp);
    f!(has_total_claimed_tokens);
    f!(cashback_earned);
    f!(total_cashback_claimed);
    f!(stable_cashback_earned);
    f!(total_stable_cashback_claimed);
    f!(reserved_tail);
    if before == after {
        println!("  (no change)");
    }
}

fn do_buy(
    svm: &mut LiteSVM,
    pump_program: &Pubkey,
    pump_fees_program: &Pubkey,
    token_program: &Pubkey,
    system_program: &Pubkey,
    global: &Pubkey,
    fee_recipient: &Pubkey,
    mint: &Pubkey,
    bonding_curve: &Pubkey,
    associated_bonding_curve: &Pubkey,
    associated_user: &Pubkey,
    user: &Keypair,
    creator_vault: &Pubkey,
    event_authority: &Pubkey,
    global_volume_accumulator: &Pubkey,
    user_volume_accumulator: &Pubkey,
    fee_config: &Pubkey,
    buyback_fee_recipient: &Pubkey,
    buy_amount: u64,
    max_sol_cost: u64,
    label: &str,
) {
    let mut data = BUY_DISCRIMINATOR.to_vec();
    data.extend_from_slice(&buy_amount.to_le_bytes());
    data.extend_from_slice(&max_sol_cost.to_le_bytes());
    data.push(1); // track_volume = true
    data.extend_from_slice(&[0u8; 3]);

    let ix = Instruction {
        program_id: *pump_program,
        accounts: vec![
            AccountMeta::new_readonly(*global, false),
            AccountMeta::new(*fee_recipient, false),
            AccountMeta::new_readonly(*mint, false),
            AccountMeta::new(*bonding_curve, false),
            AccountMeta::new(*associated_bonding_curve, false),
            AccountMeta::new(*associated_user, false),
            AccountMeta::new(user.pubkey(), true),
            AccountMeta::new_readonly(*system_program, false),
            AccountMeta::new_readonly(*token_program, false),
            AccountMeta::new(*creator_vault, false),
            AccountMeta::new_readonly(*event_authority, false),
            AccountMeta::new_readonly(*pump_program, false),
            AccountMeta::new(*global_volume_accumulator, false),
            AccountMeta::new(*user_volume_accumulator, false),
            AccountMeta::new_readonly(*fee_config, false),
            AccountMeta::new_readonly(*pump_fees_program, false),
            AccountMeta::new_readonly(
                Pubkey::find_program_address(&[b"bonding-curve-v2", mint.as_ref()], pump_program).0,
                false,
            ),
            AccountMeta::new(*buyback_fee_recipient, false),
        ],
        data,
    };

    let gv_account_before = svm.get_account(global_volume_accumulator).unwrap();
    let uv_account_before = svm.get_account(user_volume_accumulator).unwrap();
    let gv_before = decode_global_volume(&gv_account_before.data);
    let uv_before = decode_user_volume(&uv_account_before.data);
    let uv_lamports_before = uv_account_before.lamports;

    let budget_ix = set_compute_unit_limit_ix(400_000);
    let blockhash = svm.latest_blockhash();
    let msg = Message::new(&[budget_ix, ix], Some(&user.pubkey()));
    let tx = Transaction::new(&[user], msg, blockhash);

    println!("\n=== {label} ===");
    match svm.send_transaction(tx) {
        Ok(meta) => {
            println!("RESULT: SUCCESS");
            for line in &meta.logs {
                println!("  LOG: {line}");
            }
        }
        Err(e) => {
            println!("RESULT: FAILED: {:?}", e.err);
            for line in &e.meta.logs {
                println!("  LOG: {line}");
            }
            return;
        }
    }

    let uv_account_after = svm.get_account(user_volume_accumulator).unwrap();
    let gv_after = decode_global_volume(&svm.get_account(global_volume_accumulator).unwrap().data);
    let uv_after = decode_user_volume(&uv_account_after.data);
    print_global_volume_diff(label, &gv_before, &gv_after);
    print_user_volume_diff(label, &uv_before, &uv_after);
    // Real `pump` tracks bonding-curve cashback as raw lamports sitting in
    // `UserVolumeAccumulator`'s own balance (claimed later via
    // `claim_cashback`), not as a data field — per
    // `pump-public-docs/docs/PUMP_CASHBACK_README.md`. A data-only diff would
    // silently miss this.
    if uv_lamports_before != uv_account_after.lamports {
        println!(
            "  user_volume_accumulator LAMPORTS: {} -> {} (delta {})",
            uv_lamports_before,
            uv_account_after.lamports,
            uv_account_after.lamports as i64 - uv_lamports_before as i64
        );
    } else {
        println!("  user_volume_accumulator lamports: (no change, {uv_lamports_before})");
    }
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

    let now = 1_700_000_000i64;
    svm.set_sysvar::<Clock>(&Clock { unix_timestamp: now, slot: 100, epoch: 0, leader_schedule_epoch: 0, epoch_start_timestamp: 0 });

    let creator = Pubkey::new_unique();
    let mint = Pubkey::new_unique();
    svm.set_account(
        mint,
        Account { lamports: 10_000_000, data: mint_account_data(None, 6), owner: token_program, executable: false, rent_epoch: 0 },
    )
    .unwrap();

    let fee_recipient = Keypair::new();
    // A real `pump_fees::BuybackVault` PDA, not a bare wallet — confirmed via
    // `inspect_buy_tx`: `Global.buyback_fee_recipients[i]` are addresses of
    // real `BuybackVault` accounts owned by `pump_fees` (208 bytes, matching
    // that component's real field layout exactly).
    let (buyback_fee_recipient, _) =
        Pubkey::find_program_address(&[b"buyback-vault", &[0u8]], &pump_fees_program);
    svm.set_account(
        buyback_fee_recipient,
        Account {
            lamports: 10_000_000,
            data: buyback_vault_account_data(&Pubkey::new_unique()),
            owner: pump_fees_program,
            executable: false,
            rent_epoch: 0,
        },
    )
    .unwrap();
    let (global, _) = Pubkey::find_program_address(&[b"global"], &pump_program);
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

    // Plenty of reserves — this probe is about volume-accumulator behavior,
    // not graduation (already probe15's job).
    const VIRTUAL_TOKEN_RESERVES: u64 = 1_073_000_000_000_000;
    const VIRTUAL_SOL_RESERVES: u64 = 30_000_000_000;
    const REAL_TOKEN_RESERVES: u64 = 793_100_000_000_000;
    const TOKEN_TOTAL_SUPPLY: u64 = 1_000_000_000_000_000;

    let (bonding_curve, _) = Pubkey::find_program_address(&[b"bonding-curve", mint.as_ref()], &pump_program);
    svm.set_account(
        bonding_curve,
        Account {
            lamports: 10_000_000,
            data: bonding_curve_account_data(&BondingCurveFixture {
                virtual_token_reserves: VIRTUAL_TOKEN_RESERVES,
                virtual_sol_reserves: VIRTUAL_SOL_RESERVES,
                real_token_reserves: REAL_TOKEN_RESERVES,
                real_sol_reserves: 0,
                token_total_supply: TOKEN_TOTAL_SUPPLY,
                complete: false,
                creator,
                quote_mint: Pubkey::default(),
                is_cashback_coin: true,
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
        Account { lamports: 2_039_280, data: token_account_data(&mint, &user.pubkey(), 0), owner: token_program, executable: false, rent_epoch: 0 },
    )
    .unwrap();

    // Neither of these was ever funded before — both start at 0 lamports in
    // real usage too (never `init`'d, per our own `creator_vault` design;
    // `fee_recipient` here is a fresh throwaway `Keypair`), but a small fee
    // credit into a 0-lamport account still leaves it below the rent-exempt
    // minimum (~890,880 lamports for a 0-byte account), which is what
    // `InsufficientFundsForRent` was actually reporting.
    svm.airdrop(&fee_recipient.pubkey(), 10_000_000_000).unwrap();
    let (creator_vault, _) = Pubkey::find_program_address(&[b"creator-vault", creator.as_ref()], &pump_program);
    svm.set_account(
        creator_vault,
        Account { lamports: 10_000_000, data: vec![], owner: system_program, executable: false, rent_epoch: 0 },
    )
    .unwrap();
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
        Account {
            lamports: 10_000_000,
            data: user_volume_accumulator_account_data(&user.pubkey()),
            owner: pump_program,
            executable: false,
            rent_epoch: 0,
        },
    )
    .unwrap();

    let common_args = (
        &pump_program,
        &pump_fees_program,
        &token_program,
        &system_program,
        &global,
        &fee_recipient.pubkey(),
        &mint,
        &bonding_curve,
        &associated_bonding_curve,
        &associated_user,
        &user,
        &creator_vault,
        &event_authority,
        &global_volume_accumulator,
        &user_volume_accumulator,
        &fee_config,
        &buyback_fee_recipient,
    );

    macro_rules! buy {
        ($amount:expr, $max_cost:expr, $label:expr) => {
            do_buy(
                &mut svm, common_args.0, common_args.1, common_args.2, common_args.3, common_args.4, common_args.5, common_args.6,
                common_args.7, common_args.8, common_args.9, common_args.10, common_args.11, common_args.12, common_args.13,
                common_args.14, common_args.15, common_args.16, $amount, $max_cost, $label,
            )
        };
    }

    buy!(1_000_000_000, 10_000_000_000, "buy #1 (baseline, t=now)");

    // Same day, a bit later. Amount varied (not just the Clock) since an
    // identical instruction + identical blockhash produces an identical
    // signature, which litesvm rejects as `AlreadyProcessed` — this is what
    // silently no-op'd buys #2/#3 in every earlier run.
    svm.set_sysvar::<Clock>(&Clock { unix_timestamp: now + 1_000, slot: 200, epoch: 0, leader_schedule_epoch: 0, epoch_start_timestamp: 0 });
    buy!(1_000_000_001, 10_000_000_000, "buy #2 (same day, +1000s)");

    // Jump past a day boundary (86_400s is the conventional value, but we
    // don't yet know the real `seconds_in_a_day` the account stores after
    // buy #1 — jump comfortably past any plausible value).
    svm.set_sysvar::<Clock>(&Clock { unix_timestamp: now + 200_000, slot: 300, epoch: 0, leader_schedule_epoch: 0, epoch_start_timestamp: 0 });
    buy!(1_000_000_002, 10_000_000_000, "buy #3 (+200000s, past a day boundary)");
}
