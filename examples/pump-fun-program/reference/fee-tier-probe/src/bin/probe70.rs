//! Settles empirically what byte size real `pump_fees::create_fee_sharing_config`
//! actually allocates for a freshly-created `SharingConfig` account, against
//! the real deployed `pump.so` + `pump_fees.so` bytecode directly (no
//! reliance on mainnet account state, which may already have been reset/
//! updated since creation) -- confirms whether the real account is
//! exactly-sized to its initial single shareholder, or padded to some
//! larger reserved capacity.

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
// The cached `pump_fees.so` binary in `artifacts/` has an older address
// baked into it (its own `declare_id!`) than the current live
// `8rG6Zs43yJ71...` -- loading it under any other address trips Anchor's
// `DeclaredProgramIdMismatch` before any real logic runs. Loading it at its
// own embedded address (matching `probe19.rs`) is the only way to actually
// execute this binary; account-creation *logic* (what this probe measures)
// is unlikely to have changed across whatever version bump moved the
// address, even if the address itself doesn't match the current deployment.
const PUMP_FEES_PROGRAM_ID: &str = "pfeeUxB6jkeY1Hxd7CsFCAjcbHA9rWtchMGdZ6VojVZ";
const TOKEN_PROGRAM_ID: &str = "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA";
const SYSTEM_PROGRAM_ID: &str = "11111111111111111111111111111111";

const GLOBAL_DISCRIMINATOR: [u8; 8] = [167, 232, 232, 177, 200, 108, 114, 127];
const BONDING_CURVE_DISCRIMINATOR: [u8; 8] = [23, 183, 248, 55, 96, 216, 172, 96];
const CREATE_FEE_SHARING_CONFIG_DISCRIMINATOR: [u8; 8] = [195, 78, 86, 76, 111, 52, 251, 213];
const SHARING_CONFIG_DISCRIMINATOR: [u8; 8] = [216, 74, 9, 0, 56, 140, 93, 75];

fn pk(s: &str) -> Pubkey {
    Pubkey::from_str(s).unwrap()
}

fn set_compute_unit_limit_ix(units: u32) -> Instruction {
    let mut data = vec![2u8];
    data.extend_from_slice(&units.to_le_bytes());
    Instruction { program_id: pk("ComputeBudget111111111111111111111111111111"), accounts: vec![], data }
}

fn mint_account_data(decimals: u8) -> Vec<u8> {
    let mut data = vec![0u8; 82];
    data[44] = decimals;
    data[45] = 1;
    data
}

// Real, confirmed 24-field `Global` (probe21.rs), authority = whoever we
// pass so `payer` can also double as `bonding_curve.creator` below.
fn global_account_data(authority: &Pubkey) -> Vec<u8> {
    let mut data = Vec::new();
    data.extend_from_slice(&GLOBAL_DISCRIMINATOR);
    data.push(1);
    data.extend_from_slice(authority.as_ref());
    data.extend_from_slice(&[0u8; 32]);
    data.extend_from_slice(&[0u8; 8 * 5]);
    data.extend_from_slice(&[0u8; 32]);
    data.push(0);
    data.extend_from_slice(&[0u8; 8 * 2]);
    data.extend_from_slice(&[0u8; 32 * 7]);
    data.extend_from_slice(&[0u8; 32]); // set_creator_authority
    data.extend_from_slice(&[0u8; 32]); // admin_set_creator_authority
    data.push(0);
    data.extend_from_slice(&[0u8; 32]);
    data.extend_from_slice(&[0u8; 32]);
    data.push(0);
    data.extend_from_slice(&[0u8; 32 * 7]);
    data.push(0);
    data.extend_from_slice(&[0u8; 32 * 8]);
    data.extend_from_slice(&0u64.to_le_bytes());
    data.extend_from_slice(&0u64.to_le_bytes());
    data.extend_from_slice(&[0u8; 32]);
    assert_eq!(data.len(), 1045, "Global layout drifted from probe21's confirmed offsets");
    data
}

// Real `BondingCurve` (probe23.rs), `creator` set to whoever we pass.
fn bonding_curve_account_data(creator: &Pubkey) -> Vec<u8> {
    let mut data = Vec::new();
    data.extend_from_slice(&BONDING_CURVE_DISCRIMINATOR);
    data.extend_from_slice(&1_073_000_000_000_000u64.to_le_bytes()); // virtual_token_reserves
    data.extend_from_slice(&30_000_000_000u64.to_le_bytes()); // virtual_quote_reserves
    data.extend_from_slice(&793_100_000_000_000u64.to_le_bytes()); // real_token_reserves
    data.extend_from_slice(&0u64.to_le_bytes()); // real_quote_reserves
    data.extend_from_slice(&1_000_000_000_000_000u64.to_le_bytes()); // token_total_supply
    data.push(0); // complete = false
    data.extend_from_slice(creator.as_ref());
    data.push(0);
    data.push(0);
    data.extend_from_slice(&[0u8; 32]);
    data.extend_from_slice(&[0u8; 36]);
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

    let payer = Keypair::new();
    svm.airdrop(&payer.pubkey(), 10_000_000_000).unwrap();

    let mint = Pubkey::new_unique();
    svm.set_account(mint, Account { lamports: 10_000_000, data: mint_account_data(6), owner: token_program, executable: false, rent_epoch: 0 }).unwrap();

    let (global, _) = Pubkey::find_program_address(&[b"global"], &pump_program);
    svm.set_account(global, Account { lamports: 10_000_000, data: global_account_data(&payer.pubkey()), owner: pump_program, executable: false, rent_epoch: 0 }).unwrap();

    let (bonding_curve, _) = Pubkey::find_program_address(&[b"bonding-curve", mint.as_ref()], &pump_program);
    svm.set_account(
        bonding_curve,
        Account { lamports: 10_000_000, data: bonding_curve_account_data(&payer.pubkey()), owner: pump_program, executable: false, rent_epoch: 0 },
    ).unwrap();

    let (sharing_config, _) = Pubkey::find_program_address(&[b"sharing-config", mint.as_ref()], &pump_fees_program);
    let (pump_event_authority, _) = Pubkey::find_program_address(&[b"__event_authority"], &pump_program);
    let (pump_fees_event_authority, _) = Pubkey::find_program_address(&[b"__event_authority"], &pump_fees_program);

    println!("sharing_config PDA = {sharing_config}");
    println!("(before call) exists = {}", svm.get_account(&sharing_config).is_some());

    // Real account list per pump_fees.json's `create_fee_sharing_config`:
    // event_authority, program, payer, global, mint, sharing_config,
    // system_program, bonding_curve, pump_program, pump_event_authority,
    // then 3 optional trailing accounts (pool, pump_amm_program,
    // pump_amm_event_authority) -- omitted here since this probe only needs
    // the created account's own size, not the AMM-pool CPI path.
    let accounts = vec![
        AccountMeta::new_readonly(pump_fees_event_authority, false),
        AccountMeta::new_readonly(pump_fees_program, false),
        AccountMeta::new(payer.pubkey(), true),
        AccountMeta::new_readonly(global, false),
        AccountMeta::new_readonly(mint, false),
        AccountMeta::new(sharing_config, false),
        AccountMeta::new_readonly(system_program, false),
        AccountMeta::new(bonding_curve, false),
        AccountMeta::new_readonly(pump_program, false),
        AccountMeta::new_readonly(pump_event_authority, false),
        // 3 optional trailing accounts (pool, pump_amm_program,
        // pump_amm_event_authority) -- Anchor's real "None" sentinel for an
        // optional account is the calling program's own id occupying that
        // slot, not omitting the slot entirely (confirmed by the prior
        // `AccountNotEnoughKeys` failure on `pool` when omitted outright).
        AccountMeta::new_readonly(pump_fees_program, false),
        AccountMeta::new_readonly(pump_fees_program, false),
        AccountMeta::new_readonly(pump_fees_program, false),
    ];

    let ix = Instruction { program_id: pump_fees_program, accounts, data: CREATE_FEE_SHARING_CONFIG_DISCRIMINATOR.to_vec() };
    let budget_ix = set_compute_unit_limit_ix(400_000);
    let blockhash = svm.latest_blockhash();
    let msg = Message::new(&[budget_ix, ix], Some(&payer.pubkey()));
    let tx = Transaction::new(&[&payer], msg, blockhash);

    match svm.send_transaction(tx) {
        Ok(meta) => {
            println!("RESULT: SUCCESS ({} CU)", meta.compute_units_consumed);
            for line in &meta.logs {
                if line.starts_with("Program log:") {
                    println!("  LOG: {line}");
                }
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

    let account = svm.get_account(&sharing_config).expect("sharing_config should now exist");
    println!("\nsharing_config REAL created account:");
    println!("  data.len() = {} bytes", account.data.len());
    println!("  lamports = {}", account.lamports);
    println!("  owner = {}", account.owner);
    println!("  discriminator matches real SharingConfig disc? {}", account.data.len() >= 8 && account.data[0..8] == SHARING_CONFIG_DISCRIMINATOR);
    println!("  raw hex (first 128 bytes or less): {}", hex_encode(&account.data[..account.data.len().min(128)]));
}

fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}
