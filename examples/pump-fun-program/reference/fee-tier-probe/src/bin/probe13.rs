//! Probes `create_donation_fee_pda` against the real `pump_fees.so`, to
//! resolve what `docs/plan/fees-03-instructions.md` currently marks
//! `[inferred]`:
//!   - Does `pool` need to be a real/valid account when `bonding_curve` isn't
//!     graduated (`complete == false`), or is it read unconditionally /
//!     ignored?
//!   - Where does the resulting `DonationFeePda.quote_mint` come from
//!     (hardcoded WSOL? read off `pool`? read off `bonding_curve`?)
//!   - Where does `DonationFeePda.creator` come from (`bonding_curve.creator`?
//!     `sharing_config.admin`?)
//!   - Does `config_id` need to sign, or is it just address material?
//!   - What does the resulting `DonationFeePda.version` end up as?
//!
//! Strategy: set up a real ungraduated bonding curve + sharing config for a
//! fresh mint, call the real `create_donation_fee_pda` with a plain
//! (non-signing) `config_id` account and a dummy `pool` account, and inspect
//! the resulting `donation_fee_pda` account bytes against the real
//! `DonationFeePda` layout (`bump, version, config_id, base_mint, quote_mint,
//! creator, total_donated, last_crank_ts, _reserved[64]` — confirmed from
//! `pump-public-docs/idl/pump_fees.json`).

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
const PUMP_AMM_PROGRAM_ID: &str = "pAMMBay6oceH9fJKBRHGP5D4bD4sWpmSwMn52FMfXEA";
const SYSTEM_PROGRAM_ID: &str = "11111111111111111111111111111111";
const WSOL_MINT_ID: &str = "So11111111111111111111111111111111111111112";

const CREATE_DONATION_FEE_PDA_DISCRIMINATOR: [u8; 8] = [244, 139, 16, 88, 14, 255, 122, 26];
const SHARING_CONFIG_DISCRIMINATOR: [u8; 8] = [216, 74, 9, 0, 56, 140, 93, 75];
const BONDING_CURVE_DISCRIMINATOR: [u8; 8] = [23, 183, 248, 55, 96, 216, 172, 96];
const DONATION_FEE_PDA_DISCRIMINATOR: [u8; 8] = [246, 197, 96, 9, 193, 30, 93, 115];
const FEE_PROGRAM_GLOBAL_DISCRIMINATOR: [u8; 8] = [162, 165, 245, 49, 29, 37, 55, 242];
const POOL_DISCRIMINATOR: [u8; 8] = [241, 154, 109, 4, 17, 177, 109, 188];

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

// Same layout probe12.rs already validated against the real bytecode.
fn bonding_curve_account_data(complete: bool, creator: &Pubkey, quote_mint: &Pubkey) -> Vec<u8> {
    let mut data = Vec::new();
    data.extend_from_slice(&BONDING_CURVE_DISCRIMINATOR);
    data.extend_from_slice(&[0u8; 8]);
    data.extend_from_slice(&[0u8; 8]);
    data.extend_from_slice(&[0u8; 8]);
    data.extend_from_slice(&[0u8; 8]);
    data.extend_from_slice(&[0u8; 8]);
    data.push(complete as u8);
    data.extend_from_slice(creator.as_ref());
    data.push(0);
    data.push(0);
    data.extend_from_slice(quote_mint.as_ref());
    data
}

fn sharing_config_account_data(
    bump: u8,
    version: u8,
    mint: &Pubkey,
    admin: &Pubkey,
    admin_revoked: bool,
    shareholders: &[(Pubkey, u16)],
) -> Vec<u8> {
    let mut data = Vec::new();
    data.extend_from_slice(&SHARING_CONFIG_DISCRIMINATOR);
    data.push(bump);
    data.push(version);
    data.push(1);
    data.extend_from_slice(mint.as_ref());
    data.extend_from_slice(admin.as_ref());
    data.push(admin_revoked as u8);
    data.extend_from_slice(&(shareholders.len() as u32).to_le_bytes());
    for (address, share_bps) in shareholders {
        data.extend_from_slice(address.as_ref());
        data.extend_from_slice(&share_bps.to_le_bytes());
    }
    data
}

// bump, authority, disable_flags, social_claim_authority, claim_rate_limit,
// _reserved[256] — confirmed field order from pump-public-docs/idl/pump_fees.json.
fn fee_program_global_account_data(bump: u8, authority: &Pubkey) -> Vec<u8> {
    let mut data = Vec::new();
    data.extend_from_slice(&FEE_PROGRAM_GLOBAL_DISCRIMINATOR);
    data.push(bump);
    data.extend_from_slice(authority.as_ref());
    data.push(0); // disable_flags
    data.extend_from_slice(&[0u8; 32]); // social_claim_authority
    data.extend_from_slice(&0u64.to_le_bytes()); // claim_rate_limit
    data.extend_from_slice(&[0u8; 256]); // _reserved
    data
}

// pool_bump, index, creator, base_mint, quote_mint, lp_mint,
// pool_base_token_account, pool_quote_token_account, lp_supply, coin_creator,
// is_mayhem_mode, is_cashback_coin, virtual_quote_reserves(i128) — confirmed
// field order from pump-public-docs/idl/pump_amm.json.
fn pool_account_data(creator: &Pubkey, base_mint: &Pubkey, quote_mint: &Pubkey, coin_creator: &Pubkey) -> Vec<u8> {
    let mut data = Vec::new();
    data.extend_from_slice(&POOL_DISCRIMINATOR);
    data.push(0); // pool_bump
    data.extend_from_slice(&0u16.to_le_bytes()); // index
    data.extend_from_slice(creator.as_ref());
    data.extend_from_slice(base_mint.as_ref());
    data.extend_from_slice(quote_mint.as_ref());
    data.extend_from_slice(&[0u8; 32]); // lp_mint
    data.extend_from_slice(&[0u8; 32]); // pool_base_token_account
    data.extend_from_slice(&[0u8; 32]); // pool_quote_token_account
    data.extend_from_slice(&0u64.to_le_bytes()); // lp_supply
    data.extend_from_slice(coin_creator.as_ref());
    data.push(0); // is_mayhem_mode
    data.push(0); // is_cashback_coin
    data.extend_from_slice(&[0u8; 16]); // virtual_quote_reserves (i128)
    data
}

fn main() {
    let fees_program = pk(PUMP_FEES_PROGRAM_ID);
    let pump_program = pk(PUMP_PROGRAM_ID);
    let pump_amm_program = pk(PUMP_AMM_PROGRAM_ID);
    let system_program = pk(SYSTEM_PROGRAM_ID);
    let wsol_mint = pk(WSOL_MINT_ID);
    let token_program = pk("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA");

    let mut svm = LiteSVM::new();
    for (program_id, so_name) in [
        (fees_program, "pump_fees.so"),
        (pump_program, "pump.so"),
        (pump_amm_program, "pump_amm.so"),
    ] {
        let so_path = format!(
            "{}/../pump-rust-client/artifacts/{}",
            env!("CARGO_MANIFEST_DIR"),
            so_name
        );
        svm.add_program_from_file(program_id, &so_path)
            .unwrap_or_else(|e| panic!("load {so_name}: {e:?}"));
    }

    let mint = Pubkey::new_unique();
    svm.set_account(
        mint,
        Account {
            lamports: 10_000_000,
            data: mint_account_data(Some(&Pubkey::new_unique()), 6),
            owner: token_program,
            executable: false,
            rent_epoch: 0,
        },
    )
    .unwrap();

    let creator = Pubkey::new_unique();
    let (bonding_curve, _bc_bump) =
        Pubkey::find_program_address(&[b"bonding-curve", mint.as_ref()], &pump_program);
    svm.set_account(
        bonding_curve,
        Account {
            lamports: 10_000_000,
            // complete = true this run: testing with a fully internally-consistent
            // real Pool (all fields agreeing with bonding_curve/mint/wsol).
            data: bonding_curve_account_data(true, &creator, &wsol_mint),
            owner: pump_program,
            executable: false,
            rent_epoch: 0,
        },
    )
    .unwrap();

    let sc_admin = Pubkey::new_unique();
    let (sharing_config, sc_bump) =
        Pubkey::find_program_address(&[b"sharing-config", mint.as_ref()], &fees_program);
    svm.set_account(
        sharing_config,
        Account {
            lamports: 10_000_000,
            data: sharing_config_account_data(sc_bump, 1, &mint, &sc_admin, false, &[(sc_admin, 10_000)]),
            owner: fees_program,
            executable: false,
            rent_epoch: 0,
        },
    )
    .unwrap();

    let (fee_program_global, fpg_bump) =
        Pubkey::find_program_address(&[b"fee-program-global"], &fees_program);
    svm.set_account(
        fee_program_global,
        Account {
            lamports: 10_000_000,
            data: fee_program_global_account_data(fpg_bump, &Pubkey::new_unique()),
            owner: fees_program,
            executable: false,
            rent_epoch: 0,
        },
    )
    .unwrap();

    // Ground truth from find_creation_tx (donation-fee-pda-probe), reading the
    // ACTUAL transaction that created a real DonationFeePda: the `pool`
    // account it passed is exactly our pool_authority-chain derived address
    // — but get_account confirms that address doesn't exist on-chain AT ALL
    // (AccountNotFound), not even with wrong data. Every previous litesvm
    // attempt explicitly materialized SOME account there (right or wrong
    // data) via svm.set_account; testing here whether a genuinely absent
    // account at the correct derived address is what the real program
    // actually expects for ungraduated bonding curves.
    let (pool_authority, _pool_authority_bump) =
        Pubkey::find_program_address(&[b"pool-authority", mint.as_ref()], &pump_program);
    let (pool, _pool_bump) = Pubkey::find_program_address(
        &[b"pool", &0u16.to_le_bytes(), pool_authority.as_ref(), mint.as_ref(), wsol_mint.as_ref()],
        &pump_amm_program,
    );
    // Fully internally-consistent real Pool this run: creator=pool_authority
    // (matches the address derivation), base_mint=mint, quote_mint=WSOL,
    // coin_creator=bonding_curve.creator (the real post-migration convention).
    svm.set_account(
        pool,
        Account {
            lamports: 10_000_000,
            data: pool_account_data(&pool_authority, &mint, &wsol_mint, &creator),
            owner: pump_amm_program,
            executable: false,
            rent_epoch: 0,
        },
    )
    .unwrap();

    // config_id: plain address material, NOT a signer — probing whether the
    // real program requires it to sign.
    let config_id = Pubkey::new_unique();

    let (donation_fee_pda, df_bump) = Pubkey::find_program_address(
        &[b"donation-fee-pda", mint.as_ref(), config_id.as_ref()],
        &fees_program,
    );
    println!("expected donation_fee_pda bump: {df_bump}");

    println!("--- known pubkeys ---");
    println!("mint = {mint}");
    println!("creator (bonding_curve.creator) = {creator}");
    println!("sc_admin (sharing_config.admin) = {sc_admin}");
    println!("config_id = {config_id}");
    println!("pool_authority (unused now, kept for reference) = {pool_authority}");
    println!("pool (sentinel = amm_global_config) = {pool}");
    println!("wsol_mint = {wsol_mint}");
    println!("bonding_curve = {bonding_curve}");
    println!("sharing_config = {sharing_config}");
    println!("fee_program_global = {fee_program_global}");
    println!("donation_fee_pda = {donation_fee_pda}");
    println!("---------------------");

    let event_authority = Pubkey::find_program_address(&[b"__event_authority"], &fees_program).0;

    let payer = Keypair::new();
    svm.airdrop(&payer.pubkey(), 100_000_000_000).unwrap();

    let ix = Instruction {
        program_id: fees_program,
        accounts: vec![
            AccountMeta::new_readonly(event_authority, false),
            AccountMeta::new_readonly(fees_program, false),
            AccountMeta::new(payer.pubkey(), true),
            AccountMeta::new_readonly(system_program, false),
            AccountMeta::new_readonly(fee_program_global, false),
            AccountMeta::new(donation_fee_pda, false),
            AccountMeta::new_readonly(config_id, false),
            AccountMeta::new_readonly(mint, false),
            AccountMeta::new_readonly(bonding_curve, false),
            AccountMeta::new_readonly(pool, false),
            AccountMeta::new_readonly(sharing_config, false),
        ],
        data: CREATE_DONATION_FEE_PDA_DISCRIMINATOR.to_vec(),
    };

    let budget_ix = set_compute_unit_limit_ix(400_000);
    let blockhash = svm.latest_blockhash();
    let msg = Message::new(&[budget_ix, ix], Some(&payer.pubkey()));
    let tx = Transaction::new(&[&payer], msg, blockhash);

    match svm.send_transaction(tx) {
        Ok(meta) => {
            println!("RESULT: SUCCESS");
            for line in &meta.logs {
                println!("  {line}");
            }
            let account = svm.get_account(&donation_fee_pda).expect("donation_fee_pda must exist after success");
            println!("\ndonation_fee_pda raw data ({} bytes): {:02x?}", account.data.len(), account.data);
            if account.data.len() >= 8 {
                println!("discriminator matches expected: {}", account.data[0..8] == DONATION_FEE_PDA_DISCRIMINATOR);
            }
            if account.data.len() >= 8 + 2 + 32 * 4 + 16 {
                let mut off = 8;
                let bump = account.data[off]; off += 1;
                let version = account.data[off]; off += 1;
                let got_config_id = Pubkey::try_from(&account.data[off..off + 32]).unwrap(); off += 32;
                let got_base_mint = Pubkey::try_from(&account.data[off..off + 32]).unwrap(); off += 32;
                let got_quote_mint = Pubkey::try_from(&account.data[off..off + 32]).unwrap(); off += 32;
                let got_creator = Pubkey::try_from(&account.data[off..off + 32]).unwrap(); off += 32;
                let total_donated = u64::from_le_bytes(account.data[off..off + 8].try_into().unwrap()); off += 8;
                let last_crank_ts = i64::from_le_bytes(account.data[off..off + 8].try_into().unwrap());
                println!("\nParsed:");
                println!("  bump = {bump} (expected {df_bump})");
                println!("  version = {version}");
                println!("  config_id = {got_config_id} (expected {config_id}, match={})", got_config_id == config_id);
                println!("  base_mint = {got_base_mint} (expected {mint}, match={})", got_base_mint == mint);
                println!("  quote_mint = {got_quote_mint} (expected WSOL={}, match={})", wsol_mint, got_quote_mint == wsol_mint);
                println!("  creator = {got_creator} (expected sharing_config.admin={}, match={}; bonding_curve.creator={}, match={})",
                    sc_admin, got_creator == sc_admin, creator, got_creator == creator);
                println!("  total_donated = {total_donated}");
                println!("  last_crank_ts = {last_crank_ts}");
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
