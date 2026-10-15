//! Settles empirically what real `pump.so`'s `get_minimum_distributable_fee`
//! actually returns — nothing in the real IDL, the reference client, or any
//! prior probe shows the `minimum_required` formula (the IDL only declares
//! the return type `MinimumDistributableFeeEvent{minimum_required: u64,
//! distributable_fees: u64, can_distribute: bool}`, no source). Strong
//! hypothesis going in (to confirm or refute, not assume): this program's own
//! `CREATOR_VAULT_RENT_EXEMPT_MINIMUM = 890_880` constant — the same
//! rent-exempt floor `distribute_creator_fees` computes inline via
//! `Rent::get()?.try_minimum_balance(0)`. Also unconfirmed: whether
//! `can_distribute`'s threshold is inclusive (`>=`) or exclusive (`>`), and
//! whether `distributable_fees` is the raw `creator_vault` balance or already
//! `balance - minimum_required`.
//!
//! Real accounts (`pump-public-docs/idl/pump.json`): `mint` (relation:
//! `sharing_config`), `bonding_curve` (pda), `sharing_config` (pda, owned by
//! real `pump_fees`), `creator_vault` (pda, from `bonding_curve.creator`).
//! Explicitly documented "Permissionless instruction" — no signer required
//! among these four; only the outer transaction needs a fee payer.
//!
//! `BondingCurve`/`SharingConfig` fixtures reuse the exact real byte layouts
//! already confirmed elsewhere in this reference tree: `BondingCurve`'s from
//! `probe16.rs`'s own `bonding_curve_account_data`, `SharingConfig`'s field
//! order/1024-byte total size from `probe70.rs` (both mirrored, not
//! re-derived).

use litesvm::LiteSVM;
use solana_sdk::{
    account::Account,
    clock::Clock,
    instruction::{AccountMeta, Instruction},
    message::Message,
    pubkey::Address,
    signature::{Keypair, Signer},
    transaction::Transaction,
};
use std::str::FromStr;

const PUMP_PROGRAM_ID: &str = "6EF8rrecthR5Dkzon8Nwu78hRvfCKubJ14M5uBEwF6P";
const PUMP_FEES_PROGRAM_ID: &str = "pfeeUxB6jkeY1Hxd7CsFCAjcbHA9rWtchMGdZ6VojVZ";

const GET_MINIMUM_DISTRIBUTABLE_FEE_DISCRIMINATOR: [u8; 8] = [117, 225, 127, 202, 134, 95, 68, 35];
const BONDING_CURVE_DISCRIMINATOR: [u8; 8] = [23, 183, 248, 55, 96, 216, 172, 96];
const SHARING_CONFIG_DISCRIMINATOR: [u8; 8] = [216, 74, 9, 0, 56, 140, 93, 75];

// Confirmed real, deployed rent-exempt minimum for a zero-data account
// (`constants.rs`'s own `CREATOR_VAULT_RENT_EXEMPT_MINIMUM`, itself
// confirmed via `probe51.rs`).
const CREATOR_VAULT_RENT_EXEMPT_MINIMUM: u64 = 890_880;
const TOKEN_PROGRAM_ID: &str = "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA";

// Standard SPL Token `Mint` layout (82 bytes, no Anchor discriminator):
// mint_authority: COption<Address>(36), supply: u64(8), decimals: u8(1),
// is_initialized: bool(1), freeze_authority: COption<Address>(36).
fn mint_account_data() -> Vec<u8> {
    let mut data = Vec::new();
    data.extend_from_slice(&0u32.to_le_bytes()); // mint_authority: None
    data.extend_from_slice(&[0u8; 32]);
    data.extend_from_slice(&0u64.to_le_bytes()); // supply
    data.push(6); // decimals
    data.push(1); // is_initialized
    data.extend_from_slice(&0u32.to_le_bytes()); // freeze_authority: None
    data.extend_from_slice(&[0u8; 32]);
    assert_eq!(data.len(), 82);
    data
}

fn pk(s: &str) -> Address {
    Address::from_str(s).unwrap()
}

fn set_compute_unit_limit_ix(units: u32) -> Instruction {
    let mut data = vec![2u8];
    data.extend_from_slice(&units.to_le_bytes());
    Instruction { program_id: pk("ComputeBudget111111111111111111111111111111"), accounts: vec![], data }
}

struct BondingCurveFixture {
    virtual_token_reserves: u64,
    virtual_sol_reserves: u64,
    real_token_reserves: u64,
    real_sol_reserves: u64,
    token_total_supply: u64,
    complete: bool,
    creator: Address,
    quote_mint: Address,
    is_cashback_coin: bool,
}

// Verbatim copy of `probe16.rs`'s own helper (real, confirmed field order).
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

// Real `pump_fees::SharingConfig` field order (confirmed via
// `probe70.rs`/this repo's own `components/sharing_config.rs` doc comment):
// bump, version, status, mint, admin, admin_revoked, shareholders_len,
// shareholders[...]. Real account is a fixed 1024 bytes total.
fn sharing_config_account_data(bump: u8, mint: &Address, admin: &Address) -> Vec<u8> {
    let mut data = Vec::new();
    data.extend_from_slice(&SHARING_CONFIG_DISCRIMINATOR);
    data.push(bump);
    data.push(2); // version
    data.push(1); // status = Active
    data.extend_from_slice(mint.as_ref());
    data.extend_from_slice(admin.as_ref());
    data.push(0); // admin_revoked = false
    data.extend_from_slice(&1u32.to_le_bytes()); // shareholders_len = 1
    data.extend_from_slice(admin.as_ref()); // shareholders[0].address
    data.extend_from_slice(&10_000u16.to_le_bytes()); // shareholders[0].share_bps
    data.resize(1024, 0);
    data
}

fn run_call(
    svm: &mut LiteSVM,
    label: &str,
    call_index: u32,
    pump_program: Address,
    payer: &Keypair,
    mint: Address,
    bonding_curve: Address,
    sharing_config: Address,
    creator_vault: Address,
    shareholder: Address,
    creator_vault_lamports: u64,
) {
    svm.set_account(
        creator_vault,
        Account { lamports: creator_vault_lamports, data: vec![], owner: pk("11111111111111111111111111111111"), executable: false, rent_epoch: 0 },
    )
    .unwrap();

    println!("=== {label} (creator_vault lamports = {creator_vault_lamports}) ===");

    let accounts = vec![
        AccountMeta::new_readonly(mint, false),
        AccountMeta::new_readonly(bonding_curve, false),
        AccountMeta::new_readonly(sharing_config, false),
        AccountMeta::new_readonly(creator_vault, false),
        // Real bytecode's error trace revealed `get_minimum_distributable_fee`
        // shares code with `distribute_creator_fees` internally
        // (`programs/pump/src/creator/distribute_creator_fees.rs:143`,
        // `NotEnoughRemainingAccounts`) -- not documented anywhere in the real
        // IDL, which shows zero accounts beyond the 4 named ones. Needs one
        // remaining account per `sharing_config.shareholders_len` (1 here,
        // matching this fixture's single shareholder).
        AccountMeta::new_readonly(shareholder, false),
    ];
    let ix = Instruction { program_id: pump_program, accounts, data: GET_MINIMUM_DISTRIBUTABLE_FEE_DISCRIMINATOR.to_vec() };
    // Vary the CU limit per call so each transaction's instruction data (and
    // therefore its signature) is unique -- otherwise litesvm rejects repeat
    // calls against unchanged account state as `AlreadyProcessed`.
    let budget_ix = set_compute_unit_limit_ix(200_000 + call_index);
    let blockhash = svm.latest_blockhash();
    let msg = Message::new(&[budget_ix, ix], Some(&payer.pubkey()));
    let tx = Transaction::new(&[payer], msg, blockhash);

    match svm.send_transaction(tx) {
        Ok(meta) => {
            println!("  RESULT: SUCCESS ({} CU)", meta.compute_units_consumed);
            let d = &meta.return_data.data;
            if d.len() == 17 {
                let minimum_required = u64::from_le_bytes(d[0..8].try_into().unwrap());
                let distributable_fees = u64::from_le_bytes(d[8..16].try_into().unwrap());
                let can_distribute = d[16] != 0;
                println!("  minimum_required = {minimum_required}");
                println!("  distributable_fees = {distributable_fees}");
                println!("  can_distribute = {can_distribute}");
                println!("  minimum_required == CREATOR_VAULT_RENT_EXEMPT_MINIMUM ({CREATOR_VAULT_RENT_EXEMPT_MINIMUM})? {}", minimum_required == CREATOR_VAULT_RENT_EXEMPT_MINIMUM);
            } else {
                println!("  unexpected return_data len {}: {:?}", d.len(), d);
            }
            for line in &meta.logs {
                if line.starts_with("Program log:") {
                    println!("  LOG: {line}");
                }
            }
        }
        Err(e) => {
            println!("  RESULT: FAILED: {:?}", e.err);
            for line in &e.meta.logs {
                if line.starts_with("Program log:") {
                    println!("  LOG: {line}");
                }
            }
        }
    }
    println!();
}

fn main() {
    let pump_program = pk(PUMP_PROGRAM_ID);
    let pump_fees_program = pk(PUMP_FEES_PROGRAM_ID);
    let mut svm = LiteSVM::new();
    let so_path = format!("{}/../pump-rust-client/artifacts/pump.so", env!("CARGO_MANIFEST_DIR"));
    svm.add_program_from_file(pump_program, &so_path).unwrap_or_else(|e| panic!("load pump.so: {e:?}"));
    svm.set_sysvar::<Clock>(&Clock { unix_timestamp: 1_700_000_000, slot: 100, epoch: 0, leader_schedule_epoch: 0, epoch_start_timestamp: 0 });

    let payer = Keypair::new();
    svm.airdrop(&payer.pubkey(), 10_000_000_000).unwrap();

    let mint = Address::new_unique();
    let admin = Address::new_unique();

    let (bonding_curve, _) = Address::find_program_address(&[b"bonding-curve", mint.as_ref()], &pump_program);
    let (sharing_config, sharing_config_bump) = Address::find_program_address(&[b"sharing-config", mint.as_ref()], &pump_fees_program);
    // Confirmed invariant from `probe11.rs`: after `create_fee_sharing_config`'s
    // CPI into `migrate_bonding_curve_creator`, `bonding_curve.creator` is
    // reassigned to literally BE `sharing_config`'s own PDA address (not the
    // admin's pubkey) -- this instruction's `BondingCurveAndSharingConfigCreatorMismatch`
    // check enforces exactly that, and `creator_vault` is derived from it.
    let creator = sharing_config;
    let (creator_vault, _) = Address::find_program_address(&[b"creator-vault", creator.as_ref()], &pump_program);

    println!("mint={mint} creator={creator}");
    println!("bonding_curve={bonding_curve} sharing_config={sharing_config} creator_vault={creator_vault}\n");

    svm.set_account(
        bonding_curve,
        Account {
            lamports: 10_000_000,
            data: bonding_curve_account_data(&BondingCurveFixture {
                virtual_token_reserves: 0,
                virtual_sol_reserves: 0,
                real_token_reserves: 0,
                real_sol_reserves: 0,
                token_total_supply: 0,
                complete: false,
                creator,
                quote_mint: Address::default(),
                is_cashback_coin: false,
            }),
            owner: pump_program,
            executable: false,
            rent_epoch: 0,
        },
    )
    .unwrap();

    svm.set_account(
        mint,
        Account { lamports: 10_000_000, data: mint_account_data(), owner: pk(TOKEN_PROGRAM_ID), executable: false, rent_epoch: 0 },
    )
    .unwrap();

    svm.set_account(
        sharing_config,
        Account {
            lamports: 10_000_000,
            data: sharing_config_account_data(sharing_config_bump, &mint, &admin),
            owner: pump_fees_program,
            executable: false,
            rent_epoch: 0,
        },
    )
    .unwrap();

    // Sweep several creator_vault balance points around the hypothesized
    // rent-exempt threshold to nail down both the formula and the boundary
    // condition (inclusive vs exclusive) in one pass.
    // Second pass showed can_distribute was already true at
    // distributable_fees = minimum_required - 1 -- meaning can_distribute is
    // NOT distributable_fees-relative at all, it's the RAW creator_vault
    // balance compared against minimum_required directly (first pass: balance
    // 890_881 < minimum_required 1_781_760 -> false; second pass: balance
    // 2_672_639 > minimum_required -> true, consistent with a raw-balance
    // comparison). These 3 points pin that boundary exactly.
    let points: [(&str, u64); 3] = [
        ("balance = minimum_required - 1", 1_781_760 - 1),
        ("balance = minimum_required exactly", 1_781_760),
        ("balance = minimum_required + 1", 1_781_760 + 1),
    ];

    for (i, (label, lamports)) in points.into_iter().enumerate() {
        run_call(&mut svm, label, i as u32, pump_program, &payer, mint, bonding_curve, sharing_config, creator_vault, admin, lamports);
    }

    // Final check: does the shared distribute_creator_fees-style logic also
    // validate the remaining account's ADDRESS against
    // sharing_config.shareholders[i], or only the COUNT? Pass a wrong
    // (unrelated) pubkey in the shareholder slot instead of `admin`.
    let wrong_shareholder = Address::new_unique();
    run_call(&mut svm, "wrong shareholder address (expect failure if address-checked)", 10, pump_program, &payer, mint, bonding_curve, sharing_config, creator_vault, wrong_shareholder, 5_890_880);
}
