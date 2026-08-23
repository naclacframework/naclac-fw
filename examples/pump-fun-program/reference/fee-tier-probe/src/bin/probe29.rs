//! Compute-unit baseline from the real, deployed `pump.so` for `create`, same
//! purpose as `probe28`'s `buy`/`sell` baseline. Unlike `buy`/`sell`, this is
//! NOT an apples-to-apples comparison against `pump-bonding-curve`'s own
//! `create`: the real instruction also CPIs into Metaplex Token Metadata
//! (`mpl_token_metadata`) to create a `metadata` account holding
//! name/symbol/uri — genuinely more work than this reimplementation's
//! `create`, which only mints the token and creates the bonding-curve ATA.
//! Account list/discriminator taken directly from
//! `reference/pump-rust-client/idls/pump.json`'s `create` entry.

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
const MPL_TOKEN_METADATA_PROGRAM_ID: &str = "metaqbxxUerdq28cj1RbAWkYQm3ybzjb6a8bt518x1s";
const TOKEN_PROGRAM_ID: &str = "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA";
const ASSOCIATED_TOKEN_PROGRAM_ID: &str = "ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL";
const SYSTEM_PROGRAM_ID: &str = "11111111111111111111111111111111";
const RENT_SYSVAR_ID: &str = "SysvarRent111111111111111111111111111111111";

const CREATE_DISCRIMINATOR: [u8; 8] = [24, 30, 200, 40, 5, 28, 7, 119];
const GLOBAL_DISCRIMINATOR: [u8; 8] = [167, 232, 232, 177, 200, 108, 114, 127];

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

// Same shape as `probe20`/`probe22`'s `global_account_data`, just a plain
// zeroed `Global` — `create` doesn't validate `fee_recipient`/buyback state.
fn global_account_data(authority: &Pubkey) -> Vec<u8> {
    let mut data = Vec::new();
    data.extend_from_slice(&GLOBAL_DISCRIMINATOR);
    data.push(1); // initialized
    data.extend_from_slice(authority.as_ref()); // authority
    data.extend_from_slice(&[0u8; 32]); // fee_recipient
    data.extend_from_slice(&1_073_000_000_000_000u64.to_le_bytes()); // initial_virtual_token_reserves
    data.extend_from_slice(&30_000_000_000u64.to_le_bytes()); // initial_virtual_sol_reserves
    data.extend_from_slice(&793_100_000_000_000u64.to_le_bytes()); // initial_real_token_reserves
    data.extend_from_slice(&1_000_000_000_000_000u64.to_le_bytes()); // token_total_supply
    data.extend_from_slice(&0u64.to_le_bytes()); // fee_basis_points
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
    data.extend_from_slice(&[0u8; 32 * 8]); // buyback_fee_recipients[8]
    data.extend_from_slice(&0u64.to_le_bytes()); // buyback_basis_points
    data.extend_from_slice(&0u64.to_le_bytes()); // initial_virtual_quote_reserves
    data.extend_from_slice(&[0u8; 32]); // whitelisted_quote_mints[1]
    data
}

fn main() {
    let pump_program = pk(PUMP_PROGRAM_ID);
    let mpl_token_metadata_program = pk(MPL_TOKEN_METADATA_PROGRAM_ID);
    let token_program = pk(TOKEN_PROGRAM_ID);
    let associated_token_program = pk(ASSOCIATED_TOKEN_PROGRAM_ID);
    let system_program = pk(SYSTEM_PROGRAM_ID);
    let rent_sysvar = pk(RENT_SYSVAR_ID);

    let mut svm = LiteSVM::new();
    for (program_id, so_name) in [(pump_program, "pump.so"), (mpl_token_metadata_program, "mpl_token_metadata.so")] {
        let so_path = format!("{}/../pump-rust-client/artifacts/{}", env!("CARGO_MANIFEST_DIR"), so_name);
        svm.add_program_from_file(program_id, &so_path).unwrap_or_else(|e| panic!("load {so_name}: {e:?}"));
    }
    svm.set_sysvar::<Clock>(&Clock { unix_timestamp: 1_700_000_000, slot: 100, epoch: 0, leader_schedule_epoch: 0, epoch_start_timestamp: 0 });

    let user = Keypair::new();
    svm.airdrop(&user.pubkey(), 100_000_000_000).unwrap();

    let (global, _) = Pubkey::find_program_address(&[b"global"], &pump_program);
    svm.set_account(
        global,
        Account { lamports: 10_000_000, data: global_account_data(&user.pubkey()), owner: pump_program, executable: false, rent_epoch: 0 },
    )
    .unwrap();

    let mint = Keypair::new();
    let (mint_authority, _) = Pubkey::find_program_address(&[b"mint-authority"], &pump_program);
    let (bonding_curve, _) = Pubkey::find_program_address(&[b"bonding-curve", mint.pubkey().as_ref()], &pump_program);
    let associated_bonding_curve = ata_address(&bonding_curve, &mint.pubkey(), &token_program);
    let (metadata, _) = Pubkey::find_program_address(
        &[b"metadata", mpl_token_metadata_program.as_ref(), mint.pubkey().as_ref()],
        &mpl_token_metadata_program,
    );
    let (event_authority, _) = Pubkey::find_program_address(&[b"__event_authority"], &pump_program);

    let name = "Test Coin";
    let symbol = "TEST";
    let uri = "https://example.com/metadata.json";
    let mut data = CREATE_DISCRIMINATOR.to_vec();
    data.extend_from_slice(&(name.len() as u32).to_le_bytes());
    data.extend_from_slice(name.as_bytes());
    data.extend_from_slice(&(symbol.len() as u32).to_le_bytes());
    data.extend_from_slice(symbol.as_bytes());
    data.extend_from_slice(&(uri.len() as u32).to_le_bytes());
    data.extend_from_slice(uri.as_bytes());
    data.extend_from_slice(user.pubkey().as_ref()); // creator

    let ix = Instruction {
        program_id: pump_program,
        accounts: vec![
            AccountMeta::new(mint.pubkey(), true),
            AccountMeta::new_readonly(mint_authority, false),
            AccountMeta::new(bonding_curve, false),
            AccountMeta::new(associated_bonding_curve, false),
            AccountMeta::new_readonly(global, false),
            AccountMeta::new_readonly(mpl_token_metadata_program, false),
            AccountMeta::new(metadata, false),
            AccountMeta::new(user.pubkey(), true),
            AccountMeta::new_readonly(system_program, false),
            AccountMeta::new_readonly(token_program, false),
            AccountMeta::new_readonly(associated_token_program, false),
            AccountMeta::new_readonly(rent_sysvar, false),
            AccountMeta::new_readonly(event_authority, false),
            AccountMeta::new_readonly(pump_program, false),
        ],
        data,
    };

    let budget_ix = set_compute_unit_limit_ix(400_000);
    let blockhash = svm.latest_blockhash();
    let msg = Message::new(&[budget_ix, ix], Some(&user.pubkey()));
    let tx = Transaction::new(&[&user, &mint], msg, blockhash);

    match svm.send_transaction(tx) {
        Ok(meta) => println!("create: SUCCESS, {} CU", meta.compute_units_consumed),
        Err(e) => {
            println!("create: FAILED: {:?}", e.err);
            for line in &e.meta.logs {
                println!("  LOG: {line}");
            }
        }
    }
}
