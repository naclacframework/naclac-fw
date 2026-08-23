//! Follow-up to `probe31.rs`. `probe31` confirmed `pool_authority` is the
//! real payer for every account created during `migrate` (its pre-funded 20
//! SOL balance was fully drained, with any leftover swept to
//! `withdraw_authority`) — but `probe31` artificially pre-funded
//! `pool_authority` with 20 SOL to let the transaction succeed at all, so it
//! couldn't answer where that SOL comes from in the real, unmodified flow.
//! This probe answers that: `pool_authority` starts at zero lamports
//! (non-existent, not `set_account`'d) here, everything else identical to
//! `probe31`. If the transaction still succeeds, the real program must be
//! internally funding `pool_authority` from somewhere (most likely
//! `bonding_curve`'s own reserve) before using it as payer; if it fails, the
//! error identifies exactly which account was short.

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
const PUMP_AMM_PROGRAM_ID: &str = "pAMMBay6oceH9fJKBRHGP5D4bD4sWpmSwMn52FMfXEA";
const TOKEN_PROGRAM_ID: &str = "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA";
const TOKEN_2022_PROGRAM_ID: &str = "TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb";
const ASSOCIATED_TOKEN_PROGRAM_ID: &str = "ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL";
const SYSTEM_PROGRAM_ID: &str = "11111111111111111111111111111111";
const RENT_SYSVAR_ID: &str = "SysvarRent111111111111111111111111111111111";
const WSOL_MINT_ID: &str = "So11111111111111111111111111111111111111112";

const MIGRATE_DISCRIMINATOR: [u8; 8] = [155, 234, 231, 146, 236, 158, 162, 30];
const GLOBAL_DISCRIMINATOR: [u8; 8] = [167, 232, 232, 177, 200, 108, 114, 127];
const BONDING_CURVE_DISCRIMINATOR: [u8; 8] = [23, 183, 248, 55, 96, 216, 172, 96];
const GLOBAL_CONFIG_DISCRIMINATOR: [u8; 8] = [149, 8, 156, 202, 160, 252, 176, 217];

const VIRTUAL_TOKEN_RESERVES: u64 = 200_000_000_000_000;
const VIRTUAL_SOL_RESERVES: u64 = 115_005_359_056;
const REAL_TOKEN_RESERVES: u64 = 50_000_000_000_000;
const REAL_SOL_RESERVES: u64 = 85_000_000_000;
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

// Field order confirmed in `probe28`/`probe29`. `enable_migrate = 1` and
// `withdraw_authority` set to a funded account are both required for
// `migrate` to accept the call.
fn global_account_data(withdraw_authority: &Pubkey) -> Vec<u8> {
    let mut data = Vec::new();
    data.extend_from_slice(&GLOBAL_DISCRIMINATOR);
    data.push(1); // initialized
    data.extend_from_slice(&[0u8; 32]); // authority
    data.extend_from_slice(&[0u8; 32]); // fee_recipient
    data.extend_from_slice(&VIRTUAL_TOKEN_RESERVES.to_le_bytes()); // initial_virtual_token_reserves
    data.extend_from_slice(&VIRTUAL_SOL_RESERVES.to_le_bytes()); // initial_virtual_sol_reserves
    data.extend_from_slice(&REAL_TOKEN_RESERVES.to_le_bytes()); // initial_real_token_reserves
    data.extend_from_slice(&TOKEN_TOTAL_SUPPLY.to_le_bytes()); // token_total_supply
    data.extend_from_slice(&0u64.to_le_bytes()); // fee_basis_points
    data.extend_from_slice(withdraw_authority.as_ref()); // withdraw_authority
    data.push(1); // enable_migrate = true
    data.extend_from_slice(&[0u8; 8 * 2]); // pool_migration_fee, creator_fee_basis_points
    data.extend_from_slice(&[0u8; 32 * 7]); // fee_recipients[7]
    data.extend_from_slice(&[0u8; 32 * 2]); // set_creator_authority, admin_set_creator_authority
    data.push(0); // create_v2_enabled
    data.extend_from_slice(&[0u8; 32]); // whitelist_pda
    data.extend_from_slice(&[0u8; 32]); // reserved_fee_recipient
    data.push(0); // mayhem_mode_enabled
    data.extend_from_slice(&[0u8; 32 * 7]); // reserved_fee_recipients[7]
    data.push(0); // is_cashback_enabled
    data.extend_from_slice(&[0u8; 32 * 8]); // buyback_fee_recipients[8]
    data.extend_from_slice(&0u64.to_le_bytes()); // buyback_basis_points
    data.extend_from_slice(&0u64.to_le_bytes()); // initial_virtual_quote_reserves
    data.extend_from_slice(&[0u8; 32]); // whitelisted_quote_mints[1]
    data
}

fn bonding_curve_account_data(creator: &Pubkey) -> Vec<u8> {
    let mut data = Vec::new();
    data.extend_from_slice(&BONDING_CURVE_DISCRIMINATOR);
    data.extend_from_slice(&VIRTUAL_TOKEN_RESERVES.to_le_bytes());
    data.extend_from_slice(&VIRTUAL_SOL_RESERVES.to_le_bytes());
    data.extend_from_slice(&REAL_TOKEN_RESERVES.to_le_bytes());
    data.extend_from_slice(&REAL_SOL_RESERVES.to_le_bytes());
    data.extend_from_slice(&TOKEN_TOTAL_SUPPLY.to_le_bytes());
    data.push(1); // complete = true (graduated)
    data.extend_from_slice(creator.as_ref());
    data.push(0); // is_mayhem_mode
    data.push(0); // is_cashback_coin
    data.extend_from_slice(&[0u8; 32]); // quote_mint
    data
}

fn global_config_account_data() -> Vec<u8> {
    let mut data = Vec::new();
    data.extend_from_slice(&GLOBAL_CONFIG_DISCRIMINATOR);
    data.extend_from_slice(&[0u8; 32]); // admin
    data.extend_from_slice(&0u64.to_le_bytes()); // lp_fee_basis_points
    data.extend_from_slice(&0u64.to_le_bytes()); // protocol_fee_basis_points
    data.push(0); // disable_flags = 0 (create_pool enabled)
    data.extend_from_slice(&[0u8; 32 * 8]); // protocol_fee_recipients[8]
    data.extend_from_slice(&0u64.to_le_bytes()); // coin_creator_fee_basis_points
    data.extend_from_slice(&[0u8; 32]); // admin_set_coin_creator_authority
    data.extend_from_slice(&[0u8; 32]); // whitelist_pda
    data.extend_from_slice(&[0u8; 32]); // reserved_fee_recipient
    data.push(0); // mayhem_mode_enabled
    data.extend_from_slice(&[0u8; 32 * 7]); // reserved_fee_recipients[7]
    data.push(0); // is_cashback_enabled
    data.extend_from_slice(&[0u8; 32 * 8]); // buyback_fee_recipients[8]
    data.extend_from_slice(&0u64.to_le_bytes()); // buyback_basis_points
    data.extend_from_slice(&[0u8; 32]); // boost_authority
    data.push(0); // boost_enabled
    data
}

fn main() {
    let pump_program = pk(PUMP_PROGRAM_ID);
    let pump_amm_program = pk(PUMP_AMM_PROGRAM_ID);
    let token_program = pk(TOKEN_PROGRAM_ID);
    let token_2022_program = pk(TOKEN_2022_PROGRAM_ID);
    let associated_token_program = pk(ASSOCIATED_TOKEN_PROGRAM_ID);
    let system_program = pk(SYSTEM_PROGRAM_ID);
    let rent_sysvar = pk(RENT_SYSVAR_ID);
    let wsol_mint = pk(WSOL_MINT_ID);

    let mut svm = LiteSVM::new();
    for (program_id, so_name) in [(pump_program, "pump.so"), (pump_amm_program, "pump_amm.so")] {
        let so_path = format!("{}/../pump-rust-client/artifacts/{}", env!("CARGO_MANIFEST_DIR"), so_name);
        svm.add_program_from_file(program_id, &so_path).unwrap_or_else(|e| panic!("load {so_name}: {e:?}"));
    }
    svm.set_sysvar::<Clock>(&Clock { unix_timestamp: 1_700_000_000, slot: 100, epoch: 0, leader_schedule_epoch: 0, epoch_start_timestamp: 0 });

    let user = Keypair::new();
    svm.airdrop(&user.pubkey(), 10_000_000_000).unwrap();

    let withdraw_authority = Keypair::new();
    svm.airdrop(&withdraw_authority.pubkey(), 50_000_000_000).unwrap();

    let creator = Pubkey::new_unique();
    let mint = Pubkey::new_unique();
    svm.set_account(
        mint,
        Account { lamports: 10_000_000, data: mint_account_data(None, 6), owner: token_program, executable: false, rent_epoch: 0 },
    )
    .unwrap();

    svm.set_account(
        wsol_mint,
        Account { lamports: 10_000_000, data: mint_account_data(None, 9), owner: token_program, executable: false, rent_epoch: 0 },
    )
    .unwrap();

    let (global, _) = Pubkey::find_program_address(&[b"global"], &pump_program);
    svm.set_account(
        global,
        Account { lamports: 10_000_000, data: global_account_data(&withdraw_authority.pubkey()), owner: pump_program, executable: false, rent_epoch: 0 },
    )
    .unwrap();

    let (bonding_curve, _) = Pubkey::find_program_address(&[b"bonding-curve", mint.as_ref()], &pump_program);
    svm.set_account(
        bonding_curve,
        Account {
            lamports: 10_000_000 + REAL_SOL_RESERVES,
            data: bonding_curve_account_data(&creator),
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

    let (amm_global_config, _) = Pubkey::find_program_address(&[b"global_config"], &pump_amm_program);
    svm.set_account(
        amm_global_config,
        Account { lamports: 10_000_000, data: global_config_account_data(), owner: pump_amm_program, executable: false, rent_epoch: 0 },
    )
    .unwrap();

    let (pool_authority, _) = Pubkey::find_program_address(&[b"pool-authority", mint.as_ref()], &pump_program);
    // Deliberately NOT pre-funded (left non-existent) — this is the whole
    // point of this probe, see the file header.

    let index: [u8; 2] = [0, 0];
    let (pool, _) = Pubkey::find_program_address(
        &[b"pool", &index, pool_authority.as_ref(), mint.as_ref(), wsol_mint.as_ref()],
        &pump_amm_program,
    );
    let (lp_mint, _) = Pubkey::find_program_address(&[b"pool_lp_mint", pool.as_ref()], &pump_amm_program);
    let pool_authority_mint_account = ata_address(&pool_authority, &mint, &token_program);
    let pool_authority_wsol_account = ata_address(&pool_authority, &wsol_mint, &token_program);
    let user_pool_token_account = ata_address(&pool_authority, &lp_mint, &token_2022_program);
    let pool_base_token_account = ata_address(&pool, &mint, &token_program);
    let pool_quote_token_account = ata_address(&pool, &wsol_mint, &token_program);
    let pump_amm_event_authority = Pubkey::find_program_address(&[b"__event_authority"], &pump_amm_program).0;
    let event_authority = Pubkey::find_program_address(&[b"__event_authority"], &pump_program).0;

    println!("--- known pubkeys ---");
    println!("mint = {mint}");
    println!("bonding_curve = {bonding_curve}");
    println!("bonding_curve lamports before = {}", svm.get_account(&bonding_curve).unwrap().lamports);
    println!("pool_authority = {pool_authority}");
    println!(
        "pool_authority lamports before = {}",
        svm.get_account(&pool_authority).map(|a| a.lamports).unwrap_or(0)
    );
    println!("withdraw_authority = {}", withdraw_authority.pubkey());
    println!("withdraw_authority lamports before = {}", svm.get_account(&withdraw_authority.pubkey()).unwrap().lamports);
    println!("pool = {pool}");
    println!("lp_mint = {lp_mint}");
    println!("REAL_TOKEN_RESERVES = {REAL_TOKEN_RESERVES}");
    println!("REAL_SOL_RESERVES = {REAL_SOL_RESERVES}");
    println!("---------------------");

    let data = MIGRATE_DISCRIMINATOR.to_vec();

    let ix = Instruction {
        program_id: pump_program,
        accounts: vec![
            AccountMeta::new_readonly(global, false),
            AccountMeta::new(withdraw_authority.pubkey(), false),
            AccountMeta::new_readonly(mint, false),
            AccountMeta::new(bonding_curve, false),
            AccountMeta::new(associated_bonding_curve, false),
            AccountMeta::new_readonly(user.pubkey(), true),
            AccountMeta::new_readonly(system_program, false),
            AccountMeta::new_readonly(token_program, false),
            AccountMeta::new_readonly(pump_amm_program, false),
            AccountMeta::new(pool, false),
            AccountMeta::new(pool_authority, false),
            AccountMeta::new(pool_authority_mint_account, false),
            AccountMeta::new(pool_authority_wsol_account, false),
            AccountMeta::new_readonly(amm_global_config, false),
            AccountMeta::new_readonly(wsol_mint, false),
            AccountMeta::new(lp_mint, false),
            AccountMeta::new(user_pool_token_account, false),
            AccountMeta::new(pool_base_token_account, false),
            AccountMeta::new(pool_quote_token_account, false),
            AccountMeta::new_readonly(token_2022_program, false),
            AccountMeta::new_readonly(associated_token_program, false),
            AccountMeta::new_readonly(pump_amm_event_authority, false),
            AccountMeta::new_readonly(event_authority, false),
            AccountMeta::new_readonly(pump_program, false),
            AccountMeta::new_readonly(rent_sysvar, false),
        ],
        data,
    };

    let budget_ix = set_compute_unit_limit_ix(400_000);
    let blockhash = svm.latest_blockhash();
    let msg = Message::new(&[budget_ix, ix], Some(&user.pubkey()));
    let tx = Transaction::new(&[&user], msg, blockhash);

    match svm.send_transaction(tx) {
        Ok(meta) => {
            println!("RESULT: SUCCESS, {} CU", meta.compute_units_consumed);
            for line in &meta.logs {
                println!("  {line}");
            }

            println!("\n--- post-transaction state ---");
            let bc = svm.get_account(&bonding_curve).unwrap();
            println!("bonding_curve lamports after = {}, data = {:02x?}", bc.lamports, bc.data);

            if let Some(pa) = svm.get_account(&pool_authority) {
                println!("pool_authority lamports after = {}", pa.lamports);
            }
            if let Some(wa) = svm.get_account(&withdraw_authority.pubkey()) {
                println!("withdraw_authority lamports after = {}", wa.lamports);
            }
            if let Some(pam) = svm.get_account(&pool_authority_mint_account) {
                println!("pool_authority_mint_account exists, owner={}, data[64..72]={:02x?}", pam.owner, &pam.data.get(64..72));
            } else {
                println!("pool_authority_mint_account does NOT exist post-tx");
            }
            if let Some(paw) = svm.get_account(&pool_authority_wsol_account) {
                println!("pool_authority_wsol_account exists, owner={}, data[64..72]={:02x?}", paw.owner, &paw.data.get(64..72));
            } else {
                println!("pool_authority_wsol_account does NOT exist post-tx");
            }
            if let Some(pool_acc) = svm.get_account(&pool) {
                println!("pool exists, data ({} bytes) = {:02x?}", pool_acc.data.len(), pool_acc.data);
            } else {
                println!("pool does NOT exist post-tx");
            }
            if let Some(lp) = svm.get_account(&lp_mint) {
                println!("lp_mint exists, owner={}", lp.owner);
            }
            if let Some(upta) = svm.get_account(&user_pool_token_account) {
                println!("user_pool_token_account exists, data[64..72]={:02x?}", &upta.data.get(64..72));
            }
            if let Some(pbta) = svm.get_account(&pool_base_token_account) {
                println!("pool_base_token_account exists, data[64..72]={:02x?}", &pbta.data.get(64..72));
            }
            if let Some(pqta) = svm.get_account(&pool_quote_token_account) {
                println!("pool_quote_token_account exists, data[64..72]={:02x?}", &pqta.data.get(64..72));
            }
        }
        Err(e) => {
            println!("RESULT: FAILED: {:?}", e.err);
            for line in &e.meta.logs {
                println!("  {line}");
            }
        }
    }
}
