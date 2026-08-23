//! Definitively resolves `probe40.rs`'s unexplained `BuyZeroAmount` (constant
//! across every hand-constructed fixture tried — see
//! `docs/plan/bonding-curve-05-batch1-v2-instructions.md`) by eliminating
//! fixture-guessing entirely: clones the REAL `global`, `fee_config`,
//! `bonding_curve`, `mint`, and `associated_bonding_curve` accounts straight
//! from mainnet (via RPC) into litesvm, verbatim, for the exact same mint
//! (`3Dqqpf2icam1GKayjBh3baFKQLqiUwHrztnXbMmdpump`) that
//! `inspect_buy_exact_sol_in_tx.rs` found in a real, successful
//! `buy_exact_sol_in` transaction — then replays the same instruction
//! (fresh signer/associated_user of our own, since we don't have the real
//! user's private key) against that real state.
//!
//! Usage: cargo run --bin probe41 [-- <rpc-url>]

use litesvm::LiteSVM;
use solana_rpc_client::rpc_client::RpcClient;
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
use std::time::Duration;

const PUMP_PROGRAM_ID: &str = "6EF8rrecthR5Dkzon8Nwu78hRvfCKubJ14M5uBEwF6P";
const PUMP_FEES_PROGRAM_ID: &str = "pfeeUxB6jkeY1Hxd7CsFCAjcbHA9rWtchMGdZ6VojVZ";
const ASSOCIATED_TOKEN_PROGRAM_ID: &str = "ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL";
const SYSTEM_PROGRAM_ID: &str = "11111111111111111111111111111111";

// Confirmed real, successful `buy_exact_sol_in` mint from
// `inspect_buy_exact_sol_in_tx.rs`'s decoded transaction.
const REAL_MINT: &str = "3Dqqpf2icam1GKayjBh3baFKQLqiUwHrztnXbMmdpump";

const BUY_EXACT_SOL_IN_DISCRIMINATOR: [u8; 8] = [56, 252, 116, 8, 158, 223, 205, 95];
const GET_FEES_DISCRIMINATOR: [u8; 8] = [231, 37, 126, 85, 207, 91, 63, 52];

fn pk(s: &str) -> Pubkey {
    Pubkey::from_str(s).unwrap()
}

/// `solana-rpc-client` (pinned `^3`) and `litesvm`/`solana-sdk` (`*`) resolve
/// to different major versions of the underlying `solana-account`/`Pubkey`
/// crates, so `RpcClient::get_account`'s return type isn't directly usable
/// with `LiteSVM::set_account`. Converts by field access (never naming the
/// foreign `Account` type directly) plus a base58-string round-trip for
/// `owner` (guaranteed stable across both versions, unlike any
/// version-specific byte-conversion API).
fn fetch_account(client: &RpcClient, pubkey: &Pubkey) -> Result<Account, String> {
    let remote = client.get_account(pubkey).map_err(|e| format!("{e:?}"))?;
    Ok(Account {
        lamports: remote.lamports,
        data: remote.data.clone(),
        owner: Pubkey::from_str(&remote.owner.to_string()).map_err(|e| format!("{e:?}"))?,
        executable: remote.executable,
        rent_epoch: remote.rent_epoch,
    })
}

fn set_compute_unit_limit_ix(units: u32) -> Instruction {
    let mut data = vec![2u8];
    data.extend_from_slice(&units.to_le_bytes());
    Instruction { program_id: pk("ComputeBudget111111111111111111111111111111"), accounts: vec![], data }
}

fn ata_address(owner: &Pubkey, mint: &Pubkey, token_program: &Pubkey) -> Pubkey {
    Pubkey::find_program_address(&[owner.as_ref(), token_program.as_ref(), mint.as_ref()], &pk(ASSOCIATED_TOKEN_PROGRAM_ID)).0
}

/// Minimal fresh classic-Token-layout token account (165 bytes) — used only
/// for our own new `associated_user`, which doesn't need to match any real
/// on-chain account (we're not replaying the original user, just buying
/// with a fresh signer against the real, cloned curve state).
fn token_account_data(mint: &Pubkey, owner: &Pubkey, amount: u64) -> Vec<u8> {
    let mut data = vec![0u8; 165];
    data[0..32].copy_from_slice(mint.as_ref());
    data[32..64].copy_from_slice(owner.as_ref());
    data[64..72].copy_from_slice(&amount.to_le_bytes());
    data[108] = 1; // state = Initialized
    data
}

fn main() {
    let rpc_url = std::env::args().nth(1).unwrap_or_else(|| "https://api.mainnet-beta.solana.com".to_string());
    let client = RpcClient::new_with_timeout(rpc_url, Duration::from_secs(60));

    let pump_program = pk(PUMP_PROGRAM_ID);
    let pump_fees_program = pk(PUMP_FEES_PROGRAM_ID);
    let system_program = pk(SYSTEM_PROGRAM_ID);
    let mint = pk(REAL_MINT);

    println!("Fetching real accounts from mainnet...");

    let (global, _) = Pubkey::find_program_address(&[b"global"], &pump_program);
    let global_account = fetch_account(&client, &global).expect("fetch global failed");
    println!("  global: {} bytes, owner={}", global_account.data.len(), global_account.owner);

    let (fee_config, _) = Pubkey::find_program_address(&[b"fee_config", pump_program.as_ref()], &pump_fees_program);
    let fee_config_account = fetch_account(&client, &fee_config).expect("fetch fee_config failed");
    println!("  fee_config: {} bytes, owner={}", fee_config_account.data.len(), fee_config_account.owner);

    let (bonding_curve, _) = Pubkey::find_program_address(&[b"bonding-curve", mint.as_ref()], &pump_program);
    let bonding_curve_account = fetch_account(&client, &bonding_curve).expect("fetch bonding_curve failed");
    println!("  bonding_curve: {} bytes, owner={}", bonding_curve_account.data.len(), bonding_curve_account.owner);

    // Real field layout (confirmed): disc(8), virtual_token_reserves(8),
    // virtual_quote_reserves(8), real_token_reserves(8), real_quote_reserves(8),
    // token_total_supply(8), complete(1), creator(32) at [49..81).
    let creator = Pubkey::try_from(&bonding_curve_account.data[49..81]).expect("creator pubkey");
    let complete = bonding_curve_account.data[48] != 0;
    let real_token_reserves = u64::from_le_bytes(bonding_curve_account.data[24..32].try_into().unwrap());
    println!("  bonding_curve.creator = {creator}, complete = {complete}, real_token_reserves = {real_token_reserves}");
    if complete {
        println!("  WARNING: this curve has already graduated (complete=true) -- buy_exact_sol_in will likely fail with NotComplete-adjacent behavior for an unrelated reason. Re-run against a fresher mint if so.");
    }

    let mint_account = fetch_account(&client, &mint).expect("fetch mint failed");
    let token_program = mint_account.owner;
    println!("  mint: owner (real token program) = {token_program}");

    let associated_bonding_curve = ata_address(&bonding_curve, &mint, &token_program);
    let associated_bonding_curve_account = fetch_account(&client, &associated_bonding_curve).expect("fetch associated_bonding_curve failed");
    println!("  associated_bonding_curve: {} bytes", associated_bonding_curve_account.data.len());

    let (global_volume_accumulator, _) = Pubkey::find_program_address(&[b"global_volume_accumulator"], &pump_program);
    let global_volume_accumulator_account = fetch_account(&client, &global_volume_accumulator).expect("fetch global_volume_accumulator failed");
    println!("  global_volume_accumulator: {} bytes", global_volume_accumulator_account.data.len());

    // Real, confirmed offset (inspect_buy_tx.rs): buyback_fee_recipients[8] starts at 8+733.
    let buyback_start = 8 + 733;
    let buyback_fee_recipient = Pubkey::try_from(&global_account.data[buyback_start..buyback_start + 32]).expect("buyback pubkey");
    let buyback_vault_account = fetch_account(&client, &buyback_fee_recipient).expect("fetch buyback vault failed");
    println!("  buyback_fee_recipient (from real global[0]) = {buyback_fee_recipient}");

    let (bonding_curve_v2, _) = Pubkey::find_program_address(&[b"bonding-curve-v2", mint.as_ref()], &pump_program);
    let bonding_curve_v2_account = fetch_account(&client, &bonding_curve_v2).unwrap_or(Account {
        lamports: 0,
        data: vec![],
        owner: system_program,
        executable: false,
        rent_epoch: 0,
    });
    println!("  bonding_curve_v2: {} bytes (may legitimately not exist)", bonding_curve_v2_account.data.len());

    let fee_recipient = Pubkey::try_from(&global_account.data[41..73]).expect("fee_recipient pubkey");
    println!("  fee_recipient (from real global) = {fee_recipient}");

    let (creator_vault, _) = Pubkey::find_program_address(&[b"creator-vault", creator.as_ref()], &pump_program);
    let creator_vault_account = fetch_account(&client, &creator_vault).unwrap_or(Account {
        lamports: 1_000_000,
        data: vec![],
        owner: system_program,
        executable: false,
        rent_epoch: 0,
    });

    // --- Build litesvm world from real, cloned bytes ---
    let mut svm = LiteSVM::new();
    for (program_id, so_name) in [(pump_program, "pump.so"), (pump_fees_program, "pump_fees.so")] {
        let so_path = format!("{}/../pump-rust-client/artifacts/{}", env!("CARGO_MANIFEST_DIR"), so_name);
        svm.add_program_from_file(program_id, &so_path).unwrap_or_else(|e| panic!("load {so_name}: {e:?}"));
    }
    svm.set_sysvar::<Clock>(&Clock { unix_timestamp: 1_700_000_000, slot: 100, epoch: 0, leader_schedule_epoch: 0, epoch_start_timestamp: 0 });

    for (addr, acc) in [
        (global, &global_account),
        (fee_config, &fee_config_account),
        (bonding_curve, &bonding_curve_account),
        (mint, &mint_account),
        (associated_bonding_curve, &associated_bonding_curve_account),
        (global_volume_accumulator, &global_volume_accumulator_account),
        (buyback_fee_recipient, &buyback_vault_account),
    ] {
        svm.set_account(addr, acc.clone()).unwrap();
    }
    svm.set_account(bonding_curve_v2, bonding_curve_v2_account).unwrap();
    svm.set_account(creator_vault, creator_vault_account).unwrap();
    svm.set_account(fee_recipient, Account { lamports: 1_000_000, data: vec![], owner: system_program, executable: false, rent_epoch: 0 }).unwrap();

    let user = Keypair::new();
    svm.airdrop(&user.pubkey(), 100_000_000_000).unwrap();
    let associated_user = ata_address(&user.pubkey(), &mint, &token_program);
    // Real `associated_bonding_curve` is 170 bytes (Token-2022 with
    // extensions), not the bare classic-Token 165 — a hand-built 165-byte
    // account for `associated_user` would be structurally different from
    // every other real token account this mint actually has. Using the real
    // account as a byte-for-byte template (only patching owner[32..64) and
    // amount[64..72)) keeps whatever extension TLV data this mint requires
    // intact, rather than guessing it.
    let mut associated_user_data = associated_bonding_curve_account.data.clone();
    associated_user_data[32..64].copy_from_slice(user.pubkey().as_ref());
    associated_user_data[64..72].copy_from_slice(&0u64.to_le_bytes());
    svm.set_account(
        associated_user,
        Account {
            lamports: associated_bonding_curve_account.lamports,
            data: associated_user_data,
            owner: token_program,
            executable: false,
            rent_epoch: 0,
        },
    )
    .unwrap();

    // Hypothesis test: our fresh test user's `user_volume_accumulator` gets
    // created via `init_if_needed` (the one System CPI observed right before
    // every prior BuyZeroAmount failure) — a REAL trader calling this
    // successfully already has one. Seeding a real, already-populated
    // account (from the real successful tx's own trader,
    // `8R9jsYewwYmP3r2THQXGBB9MgWBkTgrxJj2xhkAKNPF9`, patched only to our
    // test user's pubkey) instead of letting it init fresh, to test whether
    // that's what's actually gating this.
    let real_user_volume_accumulator = pk("8R9jsYewwYmP3r2THQXGBB9MgWBkTgrxJj2xhkAKNPF9");
    let real_uva_account = fetch_account(&client, &real_user_volume_accumulator).expect("fetch real user_volume_accumulator failed");
    let (user_volume_accumulator, _) = Pubkey::find_program_address(&[b"user_volume_accumulator", user.pubkey().as_ref()], &pump_program);
    let mut uva_data = real_uva_account.data.clone();
    uva_data[8..40].copy_from_slice(user.pubkey().as_ref());
    svm.set_account(
        user_volume_accumulator,
        Account { lamports: real_uva_account.lamports, data: uva_data, owner: pump_program, executable: false, rent_epoch: 0 },
    )
    .unwrap();

    let (event_authority, _) = Pubkey::find_program_address(&[b"__event_authority"], &pump_program);

    let spendable_sol_in: u64 = 300_000_000; // same magnitude as the real confirmed tx
    let mut data = BUY_EXACT_SOL_IN_DISCRIMINATOR.to_vec();
    data.extend_from_slice(&spendable_sol_in.to_le_bytes());
    data.extend_from_slice(&1u64.to_le_bytes()); // min_tokens_out = 1 (was 0 every prior attempt -- untested hypothesis: require_gt!(min_tokens_out, 0) is the actual BuyZeroAmount check firing every time)
    // NOT appending a track_volume byte here: the real, successful
    // transaction `inspect_buy_exact_sol_in_tx.rs` decoded had exactly 24
    // bytes of instruction data (8 disc + 8 spendable_sol_in + 8
    // min_tokens_out) — no third field at all, despite the public IDL's
    // `args` array listing `track_volume: OptionBool` as a third arg. Testing
    // the hypothesis that the deployed binary doesn't actually take it (a
    // stale/inaccurate IDL), which would explain every prior BuyZeroAmount:
    // the extra byte was shifting/corrupting whatever Borsh reads next.

    let ix = Instruction {
        program_id: pump_program,
        accounts: vec![
            AccountMeta::new_readonly(global, false),
            AccountMeta::new(fee_recipient, false),
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

    println!("\nSubmitting buy_exact_sol_in (spendable_sol_in={spendable_sol_in}) against real cloned state...");
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
                    if ix_data.len() >= 34 && ix_data[0..8] == GET_FEES_DISCRIMINATOR {
                        let market_cap_lamports = u128::from_le_bytes(ix_data[9..25].try_into().unwrap());
                        let trade_size_lamports = u64::from_le_bytes(ix_data[25..33].try_into().unwrap());
                        println!("  get_fees CPI: market_cap_lamports={market_cap_lamports} trade_size_lamports={trade_size_lamports}");
                        println!("  >>> trade_size_lamports == spendable_sol_in ({spendable_sol_in})? {}", trade_size_lamports == spendable_sol_in);
                    }
                }
            }
            if let Some(acc) = svm.get_account(&associated_user) {
                let amount = u64::from_le_bytes(acc.data[64..72].try_into().unwrap());
                println!("  tokens_out (associated_user balance) = {amount}");
            }
        }
        Err(e) => {
            println!("RESULT: FAILED: {:?}", e.err);
            for line in &e.meta.logs {
                println!("  LOG: {line}");
            }
        }
    }
}
