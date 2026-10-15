//! Settles empirically which hidden checks real `pump.so`'s `collect_creator_fee`
//! (v1, native SOL) and `collect_creator_fee_v2` (SPL quote) actually enforce
//! beyond their real IDL's bare account list (`creator`, `creator_vault`,
//! `system_program` for v1; plus ATAs/quote_mint/quote_token_program for v2) --
//! the IDL shows zero args and no signer, but the real error list
//! (`pump-public-docs/idl/pump.json`) has several plausibly-relevant,
//! undocumented entries found by reading the FULL error list (not just this
//! instruction's declared accounts), per this session's own corrected
//! methodology:
//!   - `CreatorMigratedToSharingConfig` (6049): "creator has been migrated to
//!     sharing config, use pump_fees::reset_fee_sharing_config instead"
//!   - `UnableToDistributeCreatorVaultMigratedToSharingConfig` (6050):
//!     "creator_vault has been migrated to sharing config, use
//!     pump:distribute_creator_fees instead"
//!   - `UnableToDistributeCreatorFeesToExecutableRecipient` (6052): "The
//!     recipient account is executable, so it cannot receive lamports,
//!     remove it from the team first"
//!
//! Hypothesis: since `collect_creator_fee` has no `bonding_curve` account at
//! all, any "has this creator migrated" check must be self-contained --
//! almost certainly inspecting `creator`'s own OWNER field directly (a real
//! human creator is System-Program-owned; a migrated bonding curve's
//! `creator` field literally IS its `sharing_config`'s own address, owned by
//! `pump_fees` -- confirmed in `probe71.rs`/`probe11.rs`). Testing 3
//! scenarios against `creator` (System-owned baseline, pump_fees-owned
//! fixture, executable fixture) settles which checks are real and which
//! instruction(s) enforce them.

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
const TOKEN_PROGRAM_ID: &str = "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA";
const ASSOCIATED_TOKEN_PROGRAM_ID: &str = "ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL";
const SYSTEM_PROGRAM_ID: &str = "11111111111111111111111111111111";
const BPF_LOADER_UPGRADEABLE_ID: &str = "BPFLoaderUpgradeab1e11111111111111111111111";

const COLLECT_CREATOR_FEE_DISCRIMINATOR: [u8; 8] = [20, 22, 86, 123, 198, 28, 219, 132];
const COLLECT_CREATOR_FEE_V2_DISCRIMINATOR: [u8; 8] = [207, 17, 138, 242, 4, 34, 19, 56];
const SHARING_CONFIG_DISCRIMINATOR: [u8; 8] = [216, 74, 9, 0, 56, 140, 93, 75];

fn pk(s: &str) -> Address {
    Address::from_str(s).unwrap()
}

fn set_compute_unit_limit_ix(units: u32) -> Instruction {
    let mut data = vec![2u8];
    data.extend_from_slice(&units.to_le_bytes());
    Instruction { program_id: pk("ComputeBudget111111111111111111111111111111"), accounts: vec![], data }
}

/// Standard SPL Mint (82 bytes), same layout `probe71.rs` uses.
fn mint_account_data() -> Vec<u8> {
    let mut data = Vec::new();
    data.extend_from_slice(&0u32.to_le_bytes());
    data.extend_from_slice(&[0u8; 32]);
    data.extend_from_slice(&0u64.to_le_bytes());
    data.push(6);
    data.push(1);
    data.extend_from_slice(&0u32.to_le_bytes());
    data.extend_from_slice(&[0u8; 32]);
    data
}

/// Standard SPL Token Account (165 bytes), same layout `probe11.rs` uses.
fn token_account_data(mint: &Address, owner: &Address, amount: u64) -> Vec<u8> {
    let mut data = vec![0u8; 165];
    data[0..32].copy_from_slice(mint.as_ref());
    data[32..64].copy_from_slice(owner.as_ref());
    data[64..72].copy_from_slice(&amount.to_le_bytes());
    data[108] = 1; // state = Initialized
    data
}

/// Minimal fixture (bump/version/status/mint/admin/admin_revoked/
/// shareholders_len=0), padded to the real confirmed 1024-byte size -- only
/// used here as a stand-in for "an account owned by pump_fees", to test
/// whether `collect_creator_fee` rejects a `creator` owned by that program at
/// all; its exact shareholder contents don't matter for that question.
fn sharing_config_account_data() -> Vec<u8> {
    let mut data = Vec::new();
    data.extend_from_slice(&SHARING_CONFIG_DISCRIMINATOR);
    data.resize(1024, 0);
    data
}

fn run_v1(svm: &mut LiteSVM, label: &str, call_index: u32, pump_program: Address, payer: &Keypair, creator: Address, creator_vault_lamports: u64) {
    let (creator_vault, _) = Address::find_program_address(&[b"creator-vault", creator.as_ref()], &pump_program);
    svm.set_account(
        creator_vault,
        Account { lamports: creator_vault_lamports, data: vec![], owner: pk(SYSTEM_PROGRAM_ID), executable: false, rent_epoch: 0 },
    )
    .unwrap();

    let event_authority = Address::find_program_address(&[b"__event_authority"], &pump_program).0;

    println!("=== collect_creator_fee: {label} ===");
    let accounts = vec![
        AccountMeta::new(creator, false),
        AccountMeta::new(creator_vault, false),
        AccountMeta::new_readonly(pk(SYSTEM_PROGRAM_ID), false),
        AccountMeta::new_readonly(event_authority, false),
        AccountMeta::new_readonly(pump_program, false),
    ];
    let ix = Instruction { program_id: pump_program, accounts, data: COLLECT_CREATOR_FEE_DISCRIMINATOR.to_vec() };
    let budget_ix = set_compute_unit_limit_ix(200_000 + call_index);
    let blockhash = svm.latest_blockhash();
    let msg = Message::new(&[budget_ix, ix], Some(&payer.pubkey()));
    let tx = Transaction::new(&[payer], msg, blockhash);

    match svm.send_transaction(tx) {
        Ok(meta) => println!("  RESULT: SUCCESS ({} CU)", meta.compute_units_consumed),
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

fn run_v2(
    svm: &mut LiteSVM,
    label: &str,
    call_index: u32,
    pump_program: Address,
    payer: &Keypair,
    creator: Address,
    quote_mint: Address,
    creator_vault_amount: u64,
) {
    let (creator_vault, _) = Address::find_program_address(&[b"creator-vault", creator.as_ref()], &pump_program);
    let ata = |owner: &Address| -> Address {
        Address::find_program_address(&[owner.as_ref(), pk(TOKEN_PROGRAM_ID).as_ref(), quote_mint.as_ref()], &pk(ASSOCIATED_TOKEN_PROGRAM_ID)).0
    };
    let creator_token_account = ata(&creator);
    let creator_vault_token_account = ata(&creator_vault);

    svm.set_account(
        creator_vault,
        Account { lamports: 10_000_000, data: vec![], owner: pk(SYSTEM_PROGRAM_ID), executable: false, rent_epoch: 0 },
    )
    .unwrap();
    svm.set_account(
        creator_token_account,
        Account { lamports: 10_000_000, data: token_account_data(&quote_mint, &creator, 0), owner: pk(TOKEN_PROGRAM_ID), executable: false, rent_epoch: 0 },
    )
    .unwrap();
    svm.set_account(
        creator_vault_token_account,
        Account { lamports: 10_000_000, data: token_account_data(&quote_mint, &creator_vault, creator_vault_amount), owner: pk(TOKEN_PROGRAM_ID), executable: false, rent_epoch: 0 },
    )
    .unwrap();

    let event_authority = Address::find_program_address(&[b"__event_authority"], &pump_program).0;

    println!("=== collect_creator_fee_v2: {label} ===");
    let accounts = vec![
        AccountMeta::new(creator, false),
        AccountMeta::new(creator_token_account, false),
        AccountMeta::new(creator_vault, false),
        AccountMeta::new(creator_vault_token_account, false),
        AccountMeta::new_readonly(quote_mint, false),
        AccountMeta::new_readonly(pk(TOKEN_PROGRAM_ID), false),
        AccountMeta::new_readonly(pk(ASSOCIATED_TOKEN_PROGRAM_ID), false),
        AccountMeta::new_readonly(pk(SYSTEM_PROGRAM_ID), false),
        AccountMeta::new_readonly(event_authority, false),
        AccountMeta::new_readonly(pump_program, false),
    ];
    let ix = Instruction { program_id: pump_program, accounts, data: COLLECT_CREATOR_FEE_V2_DISCRIMINATOR.to_vec() };
    let budget_ix = set_compute_unit_limit_ix(200_000 + call_index);
    let blockhash = svm.latest_blockhash();
    let msg = Message::new(&[budget_ix, ix], Some(&payer.pubkey()));
    let tx = Transaction::new(&[payer], msg, blockhash);

    match svm.send_transaction(tx) {
        Ok(meta) => println!("  RESULT: SUCCESS ({} CU)", meta.compute_units_consumed),
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

    // --- v1 scenario A: plain System-owned creator (baseline, expect success) ---
    let plain_creator = Address::new_unique();
    svm.set_account(
        plain_creator,
        Account { lamports: 1_000_000, data: vec![], owner: pk(SYSTEM_PROGRAM_ID), executable: false, rent_epoch: 0 },
    )
    .unwrap();
    run_v1(&mut svm, "plain System-owned creator (baseline)", 0, pump_program, &payer, plain_creator, 5_000_000);

    // --- v1 scenario B: creator is itself a pump_fees-owned account (migrated) ---
    let migrated_creator = Address::new_unique();
    svm.set_account(
        migrated_creator,
        Account { lamports: 10_000_000, data: sharing_config_account_data(), owner: pump_fees_program, executable: false, rent_epoch: 0 },
    )
    .unwrap();
    run_v1(&mut svm, "creator owned by pump_fees (migrated sharing_config)", 1, pump_program, &payer, migrated_creator, 5_000_000);

    // --- v1 scenario C: creator is executable -- reuse the already-loaded
    // pump program itself as a real, litesvm-valid executable account rather
    // than fabricating one (litesvm rejects an ad hoc executable fixture
    // with `InvalidAccountData`).
    let executable_creator = pump_program;
    run_v1(&mut svm, "executable creator", 2, pump_program, &payer, executable_creator, 5_000_000);

    // --- v2 scenarios: same 3, plus a real SPL quote mint ---
    let quote_mint = Address::new_unique();
    svm.set_account(
        quote_mint,
        Account { lamports: 10_000_000, data: mint_account_data(), owner: pk(TOKEN_PROGRAM_ID), executable: false, rent_epoch: 0 },
    )
    .unwrap();

    run_v2(&mut svm, "plain System-owned creator (baseline)", 3, pump_program, &payer, plain_creator, quote_mint, 1_000_000);
    run_v2(&mut svm, "creator owned by pump_fees (migrated sharing_config)", 4, pump_program, &payer, migrated_creator, quote_mint, 1_000_000);
    run_v2(&mut svm, "executable creator", 5, pump_program, &payer, executable_creator, quote_mint, 1_000_000);
}
