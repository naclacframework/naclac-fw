//! Resolves `buy_exact_sol_in`'s fee-CPI circularity: the real IDL's own doc
//! comment gives a 4-step quote formula that needs `total_fee_bps` (from the
//! `get_fees` CPI) *before* it can compute `net_sol` — but `get_fees` itself
//! needs a `trade_size_lamports` input to pick the right fee tier, and
//! `net_sol` isn't known yet at that point for this instruction (unlike
//! `buy`, which already knows its net SOL cost up front and passes that).
//! Working hypothesis (see `docs/plan/bonding-curve-05-batch1-v2-instructions.md`):
//! `pump.so` calls `get_fees` with `trade_size_lamports = spendable_sol_in`
//! (the caller's declared budget). This probe reads the real `get_fees` CPI's
//! instruction data directly out of `TransactionMetadata::inner_instructions`
//! (same technique as `probe19.rs`'s Case A) to confirm or refute this
//! against real, deployed `pump.so` + `pump_fees.so`, across two different
//! `spendable_sol_in` values so a coincidental match can be ruled out.
//!
//! Also decodes the real `tokens_out` actually credited (via the resulting
//! `associated_user` token balance) to cross-check the full 4-step formula
//! end-to-end, not just the CPI input.

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

const BUY_EXACT_SOL_IN_DISCRIMINATOR: [u8; 8] = [56, 252, 116, 8, 158, 223, 205, 95];
const GET_FEES_DISCRIMINATOR: [u8; 8] = [231, 37, 126, 85, 207, 91, 63, 52];
const GLOBAL_DISCRIMINATOR: [u8; 8] = [167, 232, 232, 177, 200, 108, 114, 127];
const BONDING_CURVE_DISCRIMINATOR: [u8; 8] = [23, 183, 248, 55, 96, 216, 172, 96];
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

fn buyback_vault_account_data(authority: &Pubkey) -> Vec<u8> {
    let mut data = Vec::new();
    data.extend_from_slice(&BUYBACK_VAULT_DISCRIMINATOR);
    data.extend_from_slice(authority.as_ref());
    data.extend_from_slice(&[0u8; 8 + 8 + 8 + 8 + 8 + 128]);
    data
}

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
    for _ in 0..8 {
        data.extend_from_slice(buyback_fee_recipient.as_ref()); // buyback_fee_recipients[8]
    }
    data.extend_from_slice(&0u64.to_le_bytes()); // buyback_basis_points
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

// Same fixture/layout as probe19.rs — see that file's comment for the exact
// byte offsets this reuses.
fn fee_config_account_data(bump: u8) -> Vec<u8> {
    let mut data = include_bytes!("../../fixtures/fee_config.bin").to_vec();
    data[8] = bump;
    data
}

fn global_volume_accumulator_account_data() -> Vec<u8> {
    let mut data = Vec::new();
    data.extend_from_slice(&GLOBAL_VOLUME_ACCUMULATOR_DISCRIMINATOR);
    data.extend_from_slice(&[0u8; 8 * 3 + 32 + 8 * 30 + 8 * 30 + 56]);
    data
}

fn user_volume_accumulator_account_data(user: &Pubkey) -> Vec<u8> {
    let mut data = Vec::new();
    data.extend_from_slice(&USER_VOLUME_ACCUMULATOR_DISCRIMINATOR);
    data.extend_from_slice(user.as_ref());
    data.extend_from_slice(&[0u8; 1 + 8 + 8 + 8 + 8 + 1 + 8 + 8 + 8 + 8 + 31]);
    data
}

struct CaseConfig<'a> {
    label: &'a str,
    virtual_token_reserves: u64,
    virtual_sol_reserves: u64,
    real_token_reserves: u64,
    token_total_supply: u64,
    spendable_sol_in: u64,
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

    let fee_recipient = Keypair::new();
    svm.airdrop(&fee_recipient.pubkey(), 10_000_000_000).unwrap();

    // Same fix already established for classic `buy`/`sell`
    // (`fees-07-donation-relay-progress.md`, "Classic buy/sell need a
    // bonding_curve_v2 account"): `Global.buyback_fee_recipients[8]` are
    // real `pump_fees::BuybackVault` PDAs, and the real handler needs
    // `bonding_curve_v2` (readonly) then `buyback_fee_recipient` (writable)
    // appended as UNDOCUMENTED remaining accounts beyond the IDL's own
    // named list.
    let (buyback_fee_recipient, _) = Pubkey::find_program_address(&[b"buyback-vault", &[0u8]], &pump_fees_program);
    svm.set_account(
        buyback_fee_recipient,
        Account { lamports: 10_000_000, data: buyback_vault_account_data(&Pubkey::new_unique()), owner: pump_fees_program, executable: false, rent_epoch: 0 },
    )
    .unwrap();
    // Hypothesis test: classic `buy`'s `bonding_curve_v2` is genuinely
    // unused shape-parity (confirmed via probe), but `buy_exact_sol_in` is a
    // newer entry point — testing whether it actually reads live reserves
    // from this account instead of leaving it empty/uninitialized.
    let (bonding_curve_v2, _) = Pubkey::find_program_address(&[b"bonding-curve-v2", mint.as_ref()], &pump_program);
    svm.set_account(
        bonding_curve_v2,
        Account {
            lamports: 10_000_000,
            data: bonding_curve_account_data(
                cfg.virtual_token_reserves,
                cfg.virtual_sol_reserves,
                cfg.real_token_reserves,
                0,
                cfg.token_total_supply,
                &creator,
            ),
            owner: pump_program,
            executable: false,
            rent_epoch: 0,
        },
    )
    .unwrap();

    let (global, _) = Pubkey::find_program_address(&[b"global"], &pump_program);
    svm.set_account(
        global,
        Account { lamports: 10_000_000, data: global_account_data(&fee_recipient.pubkey(), &buyback_fee_recipient), owner: pump_program, executable: false, rent_epoch: 0 },
    )
    .unwrap();

    let (fee_config, fc_bump) = Pubkey::find_program_address(&[b"fee_config", pump_program.as_ref()], &pump_fees_program);
    svm.set_account(
        fee_config,
        Account { lamports: 10_000_000, data: fee_config_account_data(fc_bump), owner: pump_fees_program, executable: false, rent_epoch: 0 },
    )
    .unwrap();

    let (bonding_curve, _) = Pubkey::find_program_address(&[b"bonding-curve", mint.as_ref()], &pump_program);
    svm.set_account(
        bonding_curve,
        Account {
            lamports: 10_000_000,
            data: bonding_curve_account_data(
                cfg.virtual_token_reserves,
                cfg.virtual_sol_reserves,
                cfg.real_token_reserves,
                0,
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

    let mut data = BUY_EXACT_SOL_IN_DISCRIMINATOR.to_vec();
    data.extend_from_slice(&cfg.spendable_sol_in.to_le_bytes());
    data.extend_from_slice(&0u64.to_le_bytes()); // min_tokens_out = 0 (don't slippage-fail)
    // Real `OptionBool` is `struct OptionBool(bool)` — a plain required bool,
    // not an actual Option (confirmed via pump.json's `types[]`); one byte,
    // no None state despite the name.
    data.push(0); // track_volume = false

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
            AccountMeta::new_readonly(bonding_curve_v2, false),
            AccountMeta::new(buyback_fee_recipient, false),
        ],
        data,
    };

    println!("\n=== {} (spendable_sol_in={}) ===", cfg.label, cfg.spendable_sol_in);
    let budget_ix = set_compute_unit_limit_ix(400_000);
    let blockhash = svm.latest_blockhash();
    let msg = Message::new(&[budget_ix, ix], Some(&user.pubkey()));
    let tx = Transaction::new(&[&user], msg, blockhash);

    match svm.send_transaction(tx) {
        Ok(meta) => {
            println!("RESULT: SUCCESS, {} CU", meta.compute_units_consumed);
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
                        println!(
                            "  >>> trade_size_lamports == spendable_sol_in ({})? {}",
                            cfg.spendable_sol_in,
                            trade_size_lamports == cfg.spendable_sol_in
                        );
                    }
                }
            }

            // Cross-check: actual tokens credited to associated_user, and net
            // SOL actually paid (bonding_curve lamport delta), for full
            // end-to-end verification of the 4-step formula.
            if let Some(acc) = svm.get_account(&associated_user) {
                let amount = u64::from_le_bytes(acc.data[64..72].try_into().unwrap());
                println!("  tokens_out (associated_user balance) = {amount}");
            }
            if let Some(acc) = svm.get_account(&bonding_curve) {
                println!("  bonding_curve lamports after = {} (started at 10_000_000)", acc.lamports);
            }
        }
        Err(e) => {
            println!("RESULT: FAILED: {:?}", e.err);
            for line in &e.meta.logs {
                println!("  LOG: {line}");
            }
            // Logs alone would already show a nested CPI's "invoke [2]" line
            // if it happened before the failure — but double-check via the
            // structured inner-instruction list too, in case get_fees fired
            // and this is being missed.
            for inner_list in &e.meta.inner_instructions {
                for inner in inner_list {
                    println!("  INNER: program_id_index={} data={:02x?}", inner.instruction.program_id_index, inner.instruction.data);
                }
            }
        }
    }
}

fn main() {
    // Two different spendable_sol_in values (and different reserve configs,
    // to also vary the resulting fee tier / market cap) — a coincidental
    // match on one case wouldn't be conclusive.
    run_case(&CaseConfig {
        label: "Case 1",
        virtual_token_reserves: 1_073_000_000_000_000,
        virtual_sol_reserves: 30_000_000_000,
        real_token_reserves: 793_100_000_000_000,
        token_total_supply: 1_000_000_000_000_000,
        spendable_sol_in: 50_000_000_000,
    });
    run_case(&CaseConfig {
        label: "Case 2 (different reserves + spendable_sol_in)",
        virtual_token_reserves: 800_000_000_000_000,
        virtual_sol_reserves: 60_000_000_000,
        real_token_reserves: 600_000_000_000_000,
        token_total_supply: 1_000_000_000_000_000,
        spendable_sol_in: 90_000_000_000,
    });
}
