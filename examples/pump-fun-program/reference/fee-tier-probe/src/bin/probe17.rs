//! `probe16` still fails `BuybackFeeRecipientMissing` even with a real,
//! correctly-seeded `BuybackVault` fixture (index 0, seed formula confirmed
//! exactly correct via `verify_buyback_seed`). That means the real check is
//! very likely a *live* PDA re-derivation with some index-selection formula,
//! not a membership check against `Global`'s stored array — so passing
//! index 0's vault only works if the real formula also picks index 0 for
//! this exact transaction. This probe brute-forces all 8 indices (fresh
//! LiteSVM instance each time, since global state carries a `Clock`/slot
//! that might factor into the real selection) to find which one, if any,
//! actually succeeds.

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
const FEE_CONFIG_DISCRIMINATOR: [u8; 8] = [143, 52, 146, 187, 219, 123, 76, 155];
const GLOBAL_VOLUME_ACCUMULATOR_DISCRIMINATOR: [u8; 8] = [202, 42, 246, 43, 142, 190, 30, 255];
const USER_VOLUME_ACCUMULATOR_DISCRIMINATOR: [u8; 8] = [86, 255, 112, 14, 102, 53, 154, 250];
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

fn global_account_data(fee_recipient: &Pubkey, buyback_fee_recipients: &[Pubkey; 8]) -> Vec<u8> {
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
    for r in buyback_fee_recipients {
        data.extend_from_slice(r.as_ref());
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
    data.push(0);
    data.push(0);
    data.extend_from_slice(f.quote_mint.as_ref());
    data
}

fn fee_config_account_data(bump: u8, admin: &Pubkey) -> Vec<u8> {
    let mut data = Vec::new();
    data.extend_from_slice(&FEE_CONFIG_DISCRIMINATOR);
    data.push(bump);
    data.extend_from_slice(admin.as_ref());
    data.extend_from_slice(&0u64.to_le_bytes());
    data.extend_from_slice(&0u64.to_le_bytes());
    data.extend_from_slice(&0u64.to_le_bytes());
    data.extend_from_slice(&0u32.to_le_bytes());
    data.extend_from_slice(&0u32.to_le_bytes());
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

/// Runs one full `buy` attempt in a fresh SVM, passing `try_index`'s vault as
/// the sole remaining account (all 8 real vaults are still registered with
/// real backing data, and all 8 real addresses are still stored in `Global`,
/// matching real on-chain state exactly — only the *passed* remaining
/// account varies per attempt).
fn attempt(try_index: u8) -> bool {
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

    // All 8 real vault PDAs, registered with real backing data — matches
    // real on-chain state exactly (seed formula confirmed via
    // `verify_buyback_seed`).
    let mut vaults = [Pubkey::default(); 8];
    for (i, vault) in vaults.iter_mut().enumerate() {
        let (addr, _) = Pubkey::find_program_address(&[b"buyback-vault", &[i as u8]], &pump_fees_program);
        *vault = addr;
        svm.set_account(
            addr,
            Account { lamports: 10_000_000, data: buyback_vault_account_data(&Pubkey::new_unique()), owner: pump_fees_program, executable: false, rent_epoch: 0 },
        )
        .unwrap();
    }

    let fee_recipient = Keypair::new();
    let (global, _) = Pubkey::find_program_address(&[b"global"], &pump_program);
    svm.set_account(
        global,
        Account { lamports: 10_000_000, data: global_account_data(&fee_recipient.pubkey(), &vaults), owner: pump_program, executable: false, rent_epoch: 0 },
    )
    .unwrap();

    let (fee_config, fc_bump) = Pubkey::find_program_address(&[b"fee_config", pump_program.as_ref()], &pump_fees_program);
    svm.set_account(
        fee_config,
        Account { lamports: 10_000_000, data: fee_config_account_data(fc_bump, &Pubkey::new_unique()), owner: pump_fees_program, executable: false, rent_epoch: 0 },
    )
    .unwrap();

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
        Account { lamports: 2_039_280, data: token_account_data(&mint, &bonding_curve, REAL_TOKEN_RESERVES), owner: token_program, executable: false, rent_epoch: 0 },
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

    let buyback_fee_recipient = vaults[try_index as usize];

    let mut data = BUY_DISCRIMINATOR.to_vec();
    data.extend_from_slice(&1_000_000_000u64.to_le_bytes());
    data.extend_from_slice(&10_000_000_000u64.to_le_bytes());
    data.push(1); // track_volume = true
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
            AccountMeta::new(global_volume_accumulator, false),
            AccountMeta::new(user_volume_accumulator, false),
            AccountMeta::new_readonly(fee_config, false),
            AccountMeta::new_readonly(pump_fees_program, false),
            AccountMeta::new(buyback_fee_recipient, false),
        ],
        data,
    };

    let budget_ix = set_compute_unit_limit_ix(400_000);
    let blockhash = svm.latest_blockhash();
    let msg = Message::new(&[budget_ix, ix], Some(&user.pubkey()));
    let tx = Transaction::new(&[&user], msg, blockhash);

    println!("\n=== try_index={try_index} (vault={buyback_fee_recipient}) ===");
    match svm.send_transaction(tx) {
        Ok(meta) => {
            println!("RESULT: SUCCESS");
            for line in &meta.logs {
                println!("  LOG: {line}");
            }
            true
        }
        Err(e) => {
            println!("RESULT: FAILED: {:?}", e.err);
            for line in &e.meta.logs {
                println!("  LOG: {line}");
            }
            false
        }
    }
}

fn main() {
    for index in 0u8..8 {
        if attempt(index) {
            println!("\n>>> index={index} SUCCEEDED — this is the one to use going forward.");
            return;
        }
    }
    println!("\n>>> All 8 indices failed. The selection mechanism isn't a simple fixed-index PDA match.");
}
