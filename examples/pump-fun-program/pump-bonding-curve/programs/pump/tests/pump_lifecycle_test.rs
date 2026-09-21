//! Sequential, idempotent, cross-cluster lifecycle test for the pump
//! program — a deliberate counterpart to `pump_test.rs`, not a replacement.
//!
//! `pump_test.rs` is purely litesvm: every `#[test]` spins up a brand-new
//! isolated VM and bootstraps full fresh state from scratch, which only
//! works because litesvm gives a free fresh VM per run. On a real cluster
//! (devnet/localnet/mainnet) `Global` is real, persistent, one-time-creatable
//! state — calling `initialize` twice against the same address fails
//! outright. This file runs the whole real lifecycle (`initialize` → create
//! a pool → trade against it → ...) as ONE sequential flow against whatever
//! cluster it's pointed at, so the exact same code works identically
//! against litesvm, localnet, or devnet.
//!
//! Only truly one-time, shared state (`Global`) is checked for existence
//! first and reused if already live. Everything else (each pool) is
//! deliberately created fresh on every run, on a brand-new mint, rather
//! than persisted and reused — so a newly added instruction is always
//! exercised against a genuinely fresh pool, never leftover state from a
//! previous run that happens to already be migrated/traded/etc. Pool
//! creation itself deposits no real quote-side liquidity (see
//! `create_pool_v1`'s own doc comment), so the only recurring cost is small,
//! permanent structural rent — negligible on devnet.
//!
//! Cluster selection: edit the `CLUSTER` constant below. `provider.payer`
//! (your local `~/.config/solana/id.json` on an RPC cluster) acts as
//! `user`/`creator`/admin authority throughout — no separate throwaway
//! wallets are created or funded, since a real cluster's faucet/funding
//! story is a human's job, not this test's.
//!
//! New instructions get proven first in `pump_test.rs` (fast litesvm-only
//! iteration), then ported here once stable — this file is deliberately
//! built up incrementally, not all at once. Currently covers: `initialize`,
//! `create`, `create_v2`, `buy`, `sell`, `buy_v2`, `sell_v2` (buy then sell
//! back on both pools, classic and v2, to stay net-neutral on recoverable
//! quote SOL). Next: `migrate`, `migrate_v2`.

use naclac_client::*;
use pump_client::{
    fetch_bonding_curve, fetch_global, get_bonding_curve_pda, get_global_pda,
    get_global_volume_accumulator_pda, get_metadata_pda, get_mint_authority_pda,
    get_user_volume_accumulator_pda,
    instructions::{
        build_add_quote_mint, build_buy, build_buy_v2, build_create, build_create_v2,
        build_extend_account, build_initialize, build_migrate_v2, build_remove_quote_mint,
        build_sell, build_sell_v2, build_set_creator, build_set_params,
        build_set_reserved_fee_recipients,
        build_set_virtual_quote_reserves, build_toggle_create_v2, build_update_buyback_config,
        AddQuoteMintAccounts,
        BuyAccounts, BuyV2Accounts,
        CreateAccounts, CreateV2Accounts, ExtendAccountAccounts, InitializeAccounts,
        MigrateV2Accounts, RemoveQuoteMintAccounts, SellAccounts, SellV2Accounts, SetCreatorAccounts,
        SetParamsAccounts, SetReservedFeeRecipientsAccounts, SetVirtualQuoteReservesAccounts,
        ToggleCreateV2Accounts, UpdateBuybackConfigAccounts,
    },
    types::{BuyV2Args, MigrateV2Args, SellV2Args},
    SetParamsArgs, BONDING_CURVE_V2_SEED, BOOST_VAULT_SEED,
    BUYBACK_VAULT_SEED, CREATOR_VAULT_SEED, FEE_CONFIG_SEED, GLOBAL_CONFIG_SEED,
    GLOBAL_VOLUME_ACCUMULATOR_SEED, MPL_TOKEN_METADATA_PROGRAM_ID, POOL_AUTHORITY_SEED,
    POOL_LP_MINT_SEED, POOL_SEED, PROGRAM_ID, PUMP_AMM_PROGRAM_ID, PUMP_AUTHORITY_SEED,
    PUMP_FEES_PROGRAM_ID, USER_VOLUME_ACCUMULATOR_SEED,
};
use pump_amm_client::instructions::{build_create_config, build_toggle_boost, CreateConfigAccounts, ToggleBoostAccounts};
use std::path::PathBuf;

/// Change this to switch clusters -- "litesvm", "localnet", "devnet",
/// "mainnet", or a raw RPC URL (see `NaclacProvider::new`).
const CLUSTER: &str = "litesvm";

/// Mirrors `pump_test.rs`'s helper of the same name and shape -- asserts the
/// exact numeric `NaclacError` code, not just "any error".
fn assert_custom_code(result: Result<NaclacTransactionMetadata, NaclacClientError>, expected: u32) {
    match result {
        Err(NaclacClientError::TransactionFailed {
            instruction_err: InstructionError::Custom(code),
            ..
        }) => {
            assert_eq!(code, expected, "wrong custom error code");
        }
        other => panic!("expected a Custom({expected}) transaction failure, got {other:?}"),
    }
}

fn setup() -> NaclacProvider {
    let payer = load_node_wallet().expect("Failed to load local Solana keypair");
    let provider = NaclacProvider::new(CLUSTER, payer).expect("Failed to construct NaclacProvider");

    // Only litesvm needs its own bytecode loaded by hand -- every other
    // cluster this test targets already has these programs genuinely
    // deployed (confirmed live on devnet at the time this file was written).
    if provider.cluster == RpcCluster::Litesvm {
        let mut mpl_so = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        mpl_so.pop(); // programs
        mpl_so.pop(); // pump-bonding-curve workspace root
        mpl_so.pop(); // pump-fun-program
        mpl_so.push("reference/pump-rust-client/artifacts/mpl_token_metadata.so");
        provider
            .add_program(&MPL_TOKEN_METADATA_PROGRAM_ID, mpl_so.to_str().unwrap())
            .expect("Failed to load mpl_token_metadata.so");

        let mut pump_fees_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        pump_fees_root.pop(); // programs
        pump_fees_root.pop(); // pump-bonding-curve workspace root
        pump_fees_root.pop(); // pump-fun-program
        pump_fees_root.push("pump-fees");
        let pump_fees_so = resolve_cargo_target_dir(&pump_fees_root).join("deploy/pump_fees.so");
        provider
            .add_program(&PUMP_FEES_PROGRAM_ID, pump_fees_so.to_str().unwrap())
            .expect("Failed to load pump_fees.so");

        let mut pump_amm_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        pump_amm_root.pop(); // programs
        pump_amm_root.pop(); // pump-bonding-curve workspace root
        pump_amm_root.pop(); // pump-fun-program
        pump_amm_root.push("pump-amm");
        let pump_amm_so = resolve_cargo_target_dir(&pump_amm_root).join("deploy/pump_amm.so");
        provider
            .add_program(&PUMP_AMM_PROGRAM_ID, pump_amm_so.to_str().unwrap())
            .expect("Failed to load pump_amm.so");
    }

    provider
}

/// A dedicated trading wallet, separate from `provider.payer` -- funded
/// once, manually, by a human (same reasoning as `pump_fees_test.rs`'s own
/// `load_test_admin_keypair`: a real cluster's faucet/funding story is a
/// human's job, not this test's, and public devnet airdrop is rate-limited
/// too unreliably to depend on per-run). Reused as `user` across every
/// `buy`/`sell`/`buy_v2`/`sell_v2` call in `full_lifecycle`.
fn load_test_trader_keypair() -> Keypair {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("tests/wallets/test_trader.json");
    let file_content = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("Failed to read test trader keypair at {:?}: {}", path, e));
    let bytes: Vec<u8> = file_content
        .trim_matches(|c| c == '[' || c == ']' || c == ' ' || c == '\n' || c == '\r')
        .split(',')
        .map(|s| s.trim().parse::<u8>().unwrap())
        .collect();
    let secret_bytes: [u8; 32] = bytes[0..32].try_into().expect("keypair file too short");
    Keypair::new_from_array(secret_bytes)
}

/// Litesvm only: real `pump_amm::create_config` requires a signer matching
/// its hardcoded `ADMIN_PUBKEY`, which is the same project-generated keypair
/// `pump_fees_lifecycle_test.rs`/`pump_amm_lifecycle_test.rs` already use
/// (`tests/wallets/test_admin.json`, copied here verbatim -- see
/// `pump_amm::constants::ADMIN_PUBKEY`'s own doc comment for why the two
/// programs share one admin pubkey). On a real cluster this file never signs
/// with it -- `pump_amm::GlobalConfig` there is assumed already created by a
/// prior `pump_amm_lifecycle_test.rs` run against that same cluster.
fn load_test_admin_keypair() -> Keypair {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("tests/wallets/test_admin.json");
    let file_content = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("Failed to read test admin keypair at {:?}: {}", path, e));
    let bytes: Vec<u8> = file_content
        .trim_matches(|c| c == '[' || c == ']' || c == ' ' || c == '\n' || c == '\r')
        .split(',')
        .map(|s| s.trim().parse::<u8>().unwrap())
        .collect();
    let secret_bytes: [u8; 32] = bytes[0..32].try_into().expect("keypair file too short");
    Keypair::new_from_array(secret_bytes)
}

/// Mimic-SOL: a persistent, project-owned classic-SPL-Token mint (real
/// `create_v2` requires classic Token specifically for any quote mint --
/// confirmed via `probe64.rs` -- so this can never be Token-2022), used
/// strictly as a non-SOL quote mint for testing real graduation/migration
/// on devnet without spending real SOL. The keypair is committed to the
/// repo (`tests/wallets/mimic_sol_mint.json`) so its address stays the same
/// across runs -- unlike every other mint in this file, which is
/// deliberately fresh each run (see module comment), this one is meant to
/// be added to `Global.whitelisted_quote_mints` once and never removed, so
/// every run can reuse the same whitelisted mint instead of needing to
/// juggle the 2-slot array.
fn load_mimic_sol_mint_keypair() -> Keypair {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("tests/wallets/mimic_sol_mint.json");
    let file_content = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("Failed to read mimic-SOL mint keypair at {:?}: {}", path, e));
    let bytes: Vec<u8> = file_content
        .trim_matches(|c| c == '[' || c == ']' || c == ' ' || c == '\n' || c == '\r')
        .split(',')
        .map(|s| s.trim().parse::<u8>().unwrap())
        .collect();
    let secret_bytes: [u8; 32] = bytes[0..32].try_into().expect("keypair file too short");
    Keypair::new_from_array(secret_bytes)
}

/// Idempotent: creates the persistent mimic-SOL mint on first run, reuses
/// it (a no-op) on every run after.
fn ensure_mimic_sol_mint(provider: &NaclacProvider) -> Address {
    let mint_kp = load_mimic_sol_mint_keypair();
    let mint = mint_kp.address();
    if provider.get_account_data(&mint).is_ok() {
        return mint;
    }

    create_mint(provider, &mint_kp, &provider.payer.address(), 9)
        .expect("create mimic-SOL quote mint should succeed");
    mint
}

/// Idempotent: adds `quote_mint` to `Global.whitelisted_quote_mints` once
/// and never removes it -- unlike `pump_test.rs`'s own scoped
/// add-use-remove litesvm test, this persistent devnet mint is meant to
/// stay whitelisted indefinitely so every run can reuse it.
fn ensure_quote_mint_whitelisted(provider: &NaclacProvider, global_pda: Address, quote_mint: Address) {
    let global = fetch_global(provider, &global_pda).expect("global should be readable");
    if global.whitelisted_quote_mints.contains(&quote_mint) {
        return;
    }

    build_add_quote_mint(
        provider,
        PROGRAM_ID,
        quote_mint,
        AddQuoteMintAccounts { global: global_pda, authority: provider.payer.address() },
    )
    .log()
    .send_and_confirm()
    .expect("add_quote_mint should succeed");
}

/// Idempotent: `Global.initial_virtual_quote_reserves` has no other setter
/// (`set_params` doesn't include it -- see `set_virtual_quote_reserves.rs`'s
/// own module comment for why this instruction exists at all) and defaults
/// to 0, which would make any non-SOL-paired curve start with
/// `virtual_quote_reserves = 0` -- broken pricing, not just untested. Real
/// value, not a guess: fetched directly off the real mainnet `Global`
/// account, confirmed via `reference/fee-tier-probe/src/bin/probe64.rs`'s
/// own printed output.
const REAL_INITIAL_VIRTUAL_QUOTE_RESERVES: u64 = 4_292_000_000;
fn ensure_virtual_quote_reserves(provider: &NaclacProvider, global_pda: Address) {
    let global = fetch_global(provider, &global_pda).expect("global should be readable");
    if global.initial_virtual_quote_reserves != 0 {
        return;
    }

    build_set_virtual_quote_reserves(
        provider,
        PROGRAM_ID,
        REAL_INITIAL_VIRTUAL_QUOTE_RESERVES,
        SetVirtualQuoteReservesAccounts { global: global_pda, authority: provider.payer.address() },
    )
    .log()
    .send_and_confirm()
    .expect("set_virtual_quote_reserves should succeed");
}

/// Idempotent: grows `account` to its current compiled size if it already
/// exists but was created under an older, smaller layout -- a no-op
/// (`naclac-core`'s `resize_with_rent` early-returns once the account is
/// already the right size, confirmed in `extend_account_test.rs`) once it
/// catches up. Must run before anything does a typed, exact-size decode of
/// the account (e.g. `fetch_global`'s zero-copy `decode_pod_checked`) --
/// that decode fails outright against a real but undersized account, which
/// `ensure_global` below would otherwise misread as "not initialized yet"
/// and try to `initialize` a second time. Skipped when the account doesn't
/// exist at all yet: `extend_account` requires an existing, program-owned
/// account, and a fresh cluster's `ensure_global` creates `Global` at the
/// current size itself, needing no extension.
fn ensure_extended(provider: &NaclacProvider, account: Address) {
    if provider.get_account_data(&account).is_err() {
        return;
    }

    build_extend_account(
        provider,
        PROGRAM_ID,
        ExtendAccountAccounts {
            account,
            user: provider.payer.address(),
            system_program: SYSTEM_PROGRAM_ID,
        },
    )
    .log()
    .send_and_confirm()
    .expect("extend_account should succeed (grow or no-op) against an existing account");
}

/// Idempotent: real `initialize` can only ever succeed once against a given
/// cluster (`global` is a PDA, `init`-only) -- if it's already live, this
/// just returns its address without attempting to recreate it.
fn ensure_global(provider: &NaclacProvider) -> Address {
    let (global_pda, _) = get_global_pda(&PROGRAM_ID);
    if fetch_global(provider, &global_pda).is_ok() {
        return global_pda;
    }

    build_initialize(
        provider,
        PROGRAM_ID,
        InitializeAccounts {
            user: provider.payer.address(),
            global: global_pda,
            system_program: SYSTEM_PROGRAM_ID,
        },
    )
    .log()
    .send_and_confirm()
    .expect("initialize should succeed");

    global_pda
}

/// Idempotent: `set_params` has no `init` restriction (it can genuinely be
/// called more than once), but there's no reason to send a real transaction
/// re-setting values that are already set -- `token_total_supply != 0` is
/// used as the "already configured" signal, since real `initialize` never
/// sets it (confirmed by reading `initialize.rs` directly: it only sets
/// `initialized`/`authority`, leaving every other `Global` field at its
/// zero-init default until `set_params` runs).
///
/// `remaining_accounts[0..8]` (-> `fee_recipient`/`fee_recipients`) only
/// need to be rent-exempt addresses (confirmed via `set_params.rs`'s own
/// `require!(account.lamports() >= rent_exempt_minimum, ...)` -- no
/// ownership/PDA constraint at all, they're just recorded as addresses).
/// They can't just be `provider.payer.address()` repeated, though --
/// `authority` (a `#[account(mut)]` field, also `provider.payer.address()`)
/// would then be the same key as `remaining_accounts`, and naclac's own
/// duplicate-mutable-account check correctly rejects that (confirmed by
/// reading `naclac-macros/src/accounts.rs`'s `ConstraintDuplicateMutableAccount`
/// check directly -- it flags any `mut` account whose key is referenced
/// again elsewhere in the same instruction, a real Solana footgun
/// protection, not a bug). Uses 8 small program-owned PDA fixtures instead,
/// funded to rent-exemption once (idempotent) via a direct transfer.
fn ensure_admin_setup(provider: &NaclacProvider, global_pda: Address) -> [Address; 8] {
    let fee_recipients = ensure_fee_recipient_fixtures(provider);

    let global = fetch_global(provider, &global_pda).expect("global should exist");
    let payer_address = provider.payer.address();

    // Checked independently of `token_total_supply` below (not folded into
    // that same "first run only" block) -- a devnet `Global` from a run
    // before `create_v2_enabled` existed already has `token_total_supply`
    // set, which would otherwise skip this check forever via the early
    // return just below.
    if !bool::from(global.create_v2_enabled) {
        build_toggle_create_v2(
            provider,
            PROGRAM_ID,
            Bool::from(true),
            ToggleCreateV2Accounts { global: global_pda, authority: payer_address },
        )
        .log()
        .send_and_confirm()
        .expect("toggle_create_v2 should succeed");
    }

    // Same reasoning as `create_v2_enabled` above: checked and backfilled
    // independently of the `token_total_supply` early return, so a stale
    // devnet `Global` from before `set_creator`/`set_metaplex_creator`
    // existed still gets a real `set_creator_authority` -- otherwise
    // `set_creator` could never be called by anyone (`Address::default()`
    // has no real keypair). Preserves every other already-configured field
    // by reading it back off `global` rather than overwriting with fresh
    // defaults.
    if global.set_creator_authority != payer_address {
        build_set_params(
            provider,
            PROGRAM_ID,
            SetParamsArgs {
                initial_virtual_token_reserves: global.initial_virtual_token_reserves,
                initial_virtual_sol_reserves: global.initial_virtual_sol_reserves,
                initial_real_token_reserves: global.initial_real_token_reserves,
                token_total_supply: global.token_total_supply,
                fee_basis_points: global.fee_basis_points,
                withdraw_authority: global.withdraw_authority,
                enable_migrate: global.enable_migrate,
                pool_migration_fee: global.pool_migration_fee,
                creator_fee_basis_points: global.creator_fee_basis_points,
                set_creator_authority: payer_address,
                admin_set_creator_authority: global.admin_set_creator_authority,
                ..Default::default()
            },
            SetParamsAccounts { global: global_pda, authority: payer_address },
        )
        .remaining_accounts(
            fee_recipients
                .iter()
                .map(|address| AccountMeta { address: *address, is_signer: false, is_writable: false })
                .collect(),
        )
        .log()
        .send_and_confirm()
        .expect("set_params should succeed (set_creator_authority backfill)");
    }

    if global.token_total_supply != 0 {
        return fee_recipients;
    }

    build_set_params(
        provider,
        PROGRAM_ID,
        SetParamsArgs {
            initial_virtual_token_reserves: 1_073_000_000_000_000,
            initial_virtual_sol_reserves: 30_000_000_000,
            initial_real_token_reserves: 793_100_000_000_000,
            token_total_supply: 1_000_000_000_000_000,
            fee_basis_points: 0,
            withdraw_authority: payer_address,
            enable_migrate: Bool::from(true),
            pool_migration_fee: 30_000_000,
            creator_fee_basis_points: 0,
            set_creator_authority: payer_address,
            admin_set_creator_authority: Address::default(),
            ..Default::default()
        },
        SetParamsAccounts { global: global_pda, authority: payer_address },
    )
    .remaining_accounts(
        fee_recipients
            .iter()
            .map(|address| AccountMeta { address: *address, is_signer: false, is_writable: false })
            .collect(),
    )
    .log()
    .send_and_confirm()
    .expect("set_params should succeed");

    fee_recipients
}

/// 8 small, deterministic PDAs (owned by System, seeded off `PROGRAM_ID`)
/// used purely as `fee_recipient`/`fee_recipients` addresses -- they never
/// need to sign anything, only receive lamports (native `System::Transfer`
/// doesn't check a destination's owner/data at all), so a PDA works fine
/// without any dedicated on-chain account type of its own. Funded to
/// rent-exemption once; idempotent (skips already-funded ones).
fn ensure_fee_recipient_fixtures(provider: &NaclacProvider) -> [Address; 8] {
    let rent_exempt_minimum =
        provider.get_minimum_balance_for_rent_exemption(0).expect("get rent exemption minimum");
    let mut addresses = [Address::default(); 8];
    for i in 0..8u8 {
        let (pda, _bump) = Address::find_program_address(&[b"lifecycle-fee-recipient", &[i]], &PROGRAM_ID);
        addresses[i as usize] = pda;
        let current = provider.get_balance(&pda).unwrap_or(0);
        if current < rent_exempt_minimum {
            transfer_sol(provider, &pda, rent_exempt_minimum - current).expect("fund fee recipient fixture");
        }
    }
    addresses
}

/// Same "8 deterministic, rent-exempt System-owned PDAs" pattern as
/// `ensure_fee_recipient_fixtures`, for `set_reserved_fee_recipients`'s
/// `reserved_fee_recipient`/`reserved_fee_recipients` -- confirmed via
/// `probe68.rs` against real deployed `pump.so` that these 8 accounts get no
/// owner/seeds check at all, so a bare fixture is genuinely sufficient (no
/// cross-program dependency, unlike the buyback vaults below).
fn ensure_reserved_fee_recipient_fixtures(provider: &NaclacProvider) -> [Address; 8] {
    let rent_exempt_minimum =
        provider.get_minimum_balance_for_rent_exemption(0).expect("get rent exemption minimum");
    let mut addresses = [Address::default(); 8];
    for i in 0..8u8 {
        let (pda, _bump) = Address::find_program_address(&[b"lifecycle-reserved-fee-recipient", &[i]], &PROGRAM_ID);
        addresses[i as usize] = pda;
        let current = provider.get_balance(&pda).unwrap_or(0);
        if current < rent_exempt_minimum {
            transfer_sol(provider, &pda, rent_exempt_minimum - current).expect("fund reserved fee recipient fixture");
        }
    }
    addresses
}

/// Registers `whitelist_pda`/`reserved_fee_recipient(s)` on `pump`'s own
/// `Global` via `set_reserved_fee_recipients` -- idempotent, mirroring
/// `ensure_buyback_vaults`'s own already-configured check below.
fn ensure_reserved_fee_recipients(provider: &NaclacProvider, global_pda: Address) -> [Address; 8] {
    let addresses = ensure_reserved_fee_recipient_fixtures(provider);

    let global = fetch_global(provider, &global_pda).expect("global should be readable");
    if global.reserved_fee_recipient == addresses[0] && global.reserved_fee_recipients == addresses[1..8] {
        return addresses;
    }

    let payer_address = provider.payer.address();
    build_set_reserved_fee_recipients(
        provider,
        PROGRAM_ID,
        global.whitelist_pda,
        SetReservedFeeRecipientsAccounts { global: global_pda, authority: payer_address },
    )
    .remaining_accounts(
        addresses
            .iter()
            .map(|address| AccountMeta { address: *address, is_signer: false, is_writable: false })
            .collect(),
    )
    .log()
    .send_and_confirm()
    .expect("set_reserved_fee_recipients should succeed");

    addresses
}

/// Registers `pump_fees`'s 8 buyback vaults with `pump`'s own `Global` --
/// `update_buyback_config` is `pump`'s own instruction, but the vaults
/// themselves are `pump_fees`'s state (`initialize_buyback`, real rent-exempt
/// PDAs owned by `pump_fees`) -- that's a separate concern belonging to
/// `pump_fees`'s own lifecycle (`pump_fees_lifecycle_test.rs`), not this
/// file's job to create.
///
/// On a real cluster this only derives the expected addresses and requires
/// they already exist (a separate test binary against the same real
/// cluster already created them for real). litesvm can't rely on that --
/// each test binary gets its own fresh, isolated VM, so a real cross-program
/// dependency created by a *different* litesvm process is simply never
/// visible here -- so on litesvm this injects the same bare-fixture
/// (`provider.set_account`, no real `pump_fees` CPI) `pump_test.rs` already
/// uses for exactly this reason.
fn ensure_buyback_vaults(provider: &NaclacProvider, global_pda: Address) -> ([Address; 8], [u8; 8]) {
    let mut buyback_vaults = [Address::default(); 8];
    let mut buyback_bumps = [0u8; 8];

    for i in 0..8u8 {
        let (buyback_vault_pda, buyback_vault_bump) =
            Address::find_program_address(&[BUYBACK_VAULT_SEED, &[i]], &PUMP_FEES_PROGRAM_ID);
        buyback_vaults[i as usize] = buyback_vault_pda;
        buyback_bumps[i as usize] = buyback_vault_bump;

        if provider.cluster == RpcCluster::Litesvm {
            provider
                .set_account(&buyback_vault_pda, vec![], &PUMP_FEES_PROGRAM_ID, 890_880)
                .expect("inject buyback_vault fixture");
        } else {
            assert!(
                provider.get_account_data(&buyback_vault_pda).is_ok(),
                "buyback_vault {i} ({buyback_vault_pda}) doesn't exist yet -- run pump_fees_lifecycle_test.rs first"
            );
        }
    }

    let global = fetch_global(provider, &global_pda).expect("global should be readable");
    if global.buyback_fee_recipients == buyback_vaults {
        return (buyback_vaults, buyback_bumps);
    }

    let payer_address = provider.payer.address();
    build_update_buyback_config(
        provider,
        PROGRAM_ID,
        None,
        buyback_bumps,
        UpdateBuybackConfigAccounts { global: global_pda, authority: payer_address },
    )
    .remaining_accounts(
        buyback_vaults
            .iter()
            .map(|address| AccountMeta { address: *address, is_signer: false, is_writable: false })
            .collect(),
    )
    .log()
    .send_and_confirm()
    .expect("update_buyback_config should succeed");

    (buyback_vaults, buyback_bumps)
}

/// `buy`/`sell`/`buy_v2`/`sell_v2` all unconditionally CPI into
/// `pump_fees::get_fees` for `config_program_id = PROGRAM_ID` (`pump`'s own
/// address) -- same real requirement as `pump_fees_lifecycle_test.rs`'s own
/// `ensure_fee_config`, and the same litesvm-vs-real split as
/// `ensure_buyback_vaults` above: on a real cluster this `FeeConfig` already
/// exists for real (created by `pump_fees_lifecycle_test.rs`, a separate
/// process on litesvm whose state this VM can never see), so this only
/// injects a fixture on litesvm, mirroring `pump_test.rs`'s own
/// `setup_fee_config` helper. Uses the same real bps
/// (`protocol_fee_bps: 95, creator_fee_bps: 30`) `pump_fees_lifecycle_test.rs`
/// uses, read off a real mainnet `buy` transaction.
fn ensure_fee_config_fixture(provider: &NaclacProvider) -> Address {
    let (fee_config_pda, bump) =
        Address::find_program_address(&[FEE_CONFIG_SEED, PROGRAM_ID.as_ref()], &PUMP_FEES_PROGRAM_ID);

    if provider.cluster == RpcCluster::Litesvm {
        if provider.get_account_data(&fee_config_pda).is_err() {
            let zero_tier = pump_fees_client::FeeTier {
                market_cap_lamports_threshold: 0,
                fees: pump_fees_client::Fees { lp_fee_bps: 0, protocol_fee_bps: 0, creator_fee_bps: 0, ..Default::default() },
                ..Default::default()
            };
            let real_tier = pump_fees_client::FeeTier {
                market_cap_lamports_threshold: 0,
                fees: pump_fees_client::Fees { lp_fee_bps: 0, protocol_fee_bps: 95, creator_fee_bps: 30, ..Default::default() },
                ..Default::default()
            };
            let stable_tier = pump_fees_client::FeeTier {
                market_cap_lamports_threshold: 0,
                fees: pump_fees_client::Fees { lp_fee_bps: 0, protocol_fee_bps: 95, creator_fee_bps: 30, ..Default::default() },
                ..Default::default()
            };
            let mut fee_tiers = [zero_tier; 50];
            fee_tiers[0] = real_tier;
            let mut stable_fee_tiers = [zero_tier; 50];
            stable_fee_tiers[0] = stable_tier;
            let cfg = pump_fees_client::FeeConfig {
                flat_fees: real_tier.fees,
                fee_tiers,
                stable_fee_tiers,
                fee_tiers_len: 1,
                stable_fee_tiers_len: 1,
                bump,
                admin: provider.payer.address(),
                ..Default::default()
            };
            let mut data = pump_fees_client::FEECONFIG_DISCRIMINATOR.to_vec();
            data.extend_from_slice(bytemuck::bytes_of(&cfg));
            provider
                .set_account(&fee_config_pda, data, &PUMP_FEES_PROGRAM_ID, 10_000_000_000)
                .expect("inject fee_config fixture");
        }
    } else {
        let fee_config = pump_fees_client::fetch_fee_config(provider, &fee_config_pda)
            .expect("fee_config doesn't exist yet -- run pump_fees_lifecycle_test.rs first");
        assert!(
            fee_config.fee_tiers_len > 0,
            "fee_config.fee_tiers is empty -- run pump_fees_lifecycle_test.rs's ensure_fee_config first"
        );
        // `is_new_quote_mint` (any non-SOL-quote `buy_v2`/`sell_v2` trade)
        // selects `stable_fee_tiers` instead of `fee_tiers` -- confirmed via
        // `reference/fee-tier-probe/src/bin/probe5.rs` against real deployed
        // `pump_fees.so`. Without this check, a non-SOL-quote trade against
        // an empty `stable_fee_tiers` table fails deep inside the trade with
        // an opaque `NoFeeTiers`, not here where the real cause is clear.
        assert!(
            fee_config.stable_fee_tiers_len > 0,
            "fee_config.stable_fee_tiers is empty -- run pump_fees_lifecycle_test.rs's ensure_fee_config \
             first (it now populates both tables); any non-SOL-quote buy_v2/sell_v2 trade needs this"
        );
    }

    fee_config_pda
}

/// Deliberately NOT idempotent -- always creates a brand-new pool on a
/// fresh mint. Creation itself deposits no real quote-side liquidity (the
/// starting price comes entirely from `Global`'s fixed virtual-reserve
/// constants, never from anyone's deposit), so the only real cost per run
/// is the small, permanent (no `close` instruction exists, real protocol or
/// ours) structural rent for the mint/bonding_curve/ATA/metadata accounts
/// -- confirmed negligible on devnet. This keeps every run exercising a
/// truly fresh pool, so a newly added instruction can never accidentally
/// pass only because it's running against already-migrated/already-traded
/// leftover state from a previous run.
/// Passes `creator = Address::default()` deliberately (rather than
/// `provider.payer.address()` directly) so the caller has a genuine unset
/// creator to exercise `set_creator`'s real backfill logic against, right
/// after this returns -- see `full_lifecycle`'s own `set_creator` call.
fn create_pool_v1(provider: &NaclacProvider, global_pda: Address) -> (Address, Keypair) {
    let mint = Keypair::new();
    let (bonding_curve_pda, bonding_curve_bump) = get_bonding_curve_pda(&PROGRAM_ID, &mint.address());

    let (mint_authority_pda, _) = get_mint_authority_pda(&PROGRAM_ID);
    let (associated_bonding_curve_pda, _) = Address::find_program_address(
        &[bonding_curve_pda.as_ref(), TOKEN_PROGRAM_ID.as_ref(), mint.address().as_ref()],
        &ASSOCIATED_TOKEN_PROGRAM_ID,
    );
    let (metadata_pda, metadata_bump) = get_metadata_pda(&mint.address());

    build_create(
        provider,
        PROGRAM_ID,
        "Lifecycle Pool V1".to_string(),
        "LCV1".to_string(),
        "https://example.com/lifecycle-v1.json".to_string(),
        Address::default(),
        bonding_curve_bump,
        metadata_bump,
        CreateAccounts {
            user: provider.payer.address(),
            mint_authority: mint_authority_pda,
            mint: mint.address(),
            bonding_curve: bonding_curve_pda,
            associated_bonding_curve: associated_bonding_curve_pda,
            global: global_pda,
            mpl_token_metadata: MPL_TOKEN_METADATA_PROGRAM_ID,
            metadata: metadata_pda,
            system_program: SYSTEM_PROGRAM_ID,
            token_program: TOKEN_PROGRAM_ID,
            associated_token_program: ASSOCIATED_TOKEN_PROGRAM_ID,
        },
    )
    .signer(&mint)
    .log()
    .send_and_confirm()
    .expect("create should succeed");

    (bonding_curve_pda, mint)
}

/// Deliberately not idempotent, same reasoning as `create_pool_v1`.
fn create_pool_v2(provider: &NaclacProvider, global_pda: Address) -> (Address, Keypair) {
    let mint = Keypair::new();
    let (bonding_curve_pda, bonding_curve_bump) = get_bonding_curve_pda(&PROGRAM_ID, &mint.address());

    let (mint_authority_pda, _) = get_mint_authority_pda(&PROGRAM_ID);
    let (associated_bonding_curve_pda, _) = Address::find_program_address(
        &[bonding_curve_pda.as_ref(), TOKEN_2022_PROGRAM_ID.as_ref(), mint.address().as_ref()],
        &ASSOCIATED_TOKEN_PROGRAM_ID,
    );

    build_create_v2(
        provider,
        PROGRAM_ID,
        "Lifecycle Pool V2".to_string(),
        "LCV2".to_string(),
        "https://example.com/lifecycle-v2.json".to_string(),
        provider.payer.address(),
        Bool::from(false),
        bonding_curve_bump,
        CreateV2Accounts {
            user: provider.payer.address(),
            mint: mint.address(),
            mint_authority: mint_authority_pda,
            bonding_curve: bonding_curve_pda,
            associated_bonding_curve: associated_bonding_curve_pda,
            global: global_pda,
            system_program: SYSTEM_PROGRAM_ID,
            token_program: TOKEN_2022_PROGRAM_ID,
            associated_token_program: ASSOCIATED_TOKEN_PROGRAM_ID,
            quote_mint: None,
            quote_token_program: None,
            associated_quote_bonding_curve: None,
        },
    )
    .signer(&mint)
    .log()
    .send_and_confirm()
    .expect("create_v2 should succeed");

    (bonding_curve_pda, mint)
}

/// Same as `create_pool_v2`, but passes `create_v2`'s 3 optional non-SOL-quote
/// accounts (naclac `Option<T>` fields, not `remaining_accounts` -- see
/// `create_v2.rs`'s module comment) and returns the raw `Result` instead of
/// `.expect()`-ing it, so callers can assert either a successful
/// (whitelisted) or failing (unwhitelisted) outcome -- mirrors
/// `pump_test.rs`'s `create_bonding_curve_v2_with_quote_mint` exactly, ported
/// here per this file's own "prove it in `pump_test.rs` first, then port"
/// convention.
fn create_pool_v2_with_quote_mint(
    provider: &NaclacProvider,
    global_pda: Address,
    quote_mint: Option<Address>,
) -> (Result<NaclacTransactionMetadata, NaclacClientError>, Address, Keypair) {
    let mint = Keypair::new();
    let (bonding_curve_pda, bonding_curve_bump) = get_bonding_curve_pda(&PROGRAM_ID, &mint.address());

    let (mint_authority_pda, _) = get_mint_authority_pda(&PROGRAM_ID);
    let (associated_bonding_curve_pda, _) = Address::find_program_address(
        &[bonding_curve_pda.as_ref(), TOKEN_2022_PROGRAM_ID.as_ref(), mint.address().as_ref()],
        &ASSOCIATED_TOKEN_PROGRAM_ID,
    );
    let associated_quote_bonding_curve_pda = quote_mint.map(|qm| {
        Address::find_program_address(
            &[bonding_curve_pda.as_ref(), TOKEN_PROGRAM_ID.as_ref(), qm.as_ref()],
            &ASSOCIATED_TOKEN_PROGRAM_ID,
        )
        .0
    });

    let result = build_create_v2(
        provider,
        PROGRAM_ID,
        "Lifecycle Pool V2 Quote".to_string(),
        "LCV2Q".to_string(),
        "https://example.com/lifecycle-v2-quote.json".to_string(),
        provider.payer.address(),
        Bool::from(false),
        bonding_curve_bump,
        CreateV2Accounts {
            user: provider.payer.address(),
            mint: mint.address(),
            mint_authority: mint_authority_pda,
            bonding_curve: bonding_curve_pda,
            associated_bonding_curve: associated_bonding_curve_pda,
            global: global_pda,
            system_program: SYSTEM_PROGRAM_ID,
            token_program: TOKEN_2022_PROGRAM_ID,
            associated_token_program: ASSOCIATED_TOKEN_PROGRAM_ID,
            quote_mint,
            quote_token_program: quote_mint.map(|_| TOKEN_PROGRAM_ID),
            associated_quote_bonding_curve: associated_quote_bonding_curve_pda,
        },
    )
    .signer(&mint)
    .log()
    .send_and_confirm();

    (result, bonding_curve_pda, mint)
}

fn wsol_mint_address() -> Address {
    "So11111111111111111111111111111111111111112"
        .parse()
        .expect("WSOL mint address must parse")
}

/// Real on every non-litesvm cluster; litesvm starts blank, so this injects
/// the same bare fixture `pump_test.rs` already uses for the same reason.
fn ensure_wsol_mint(provider: &NaclacProvider) -> Address {
    let wsol_mint = wsol_mint_address();
    if provider.cluster == RpcCluster::Litesvm && provider.get_account_data(&wsol_mint).is_err() {
        let mut data = vec![0u8; 82];
        data[44] = 9; // decimals
        data[45] = 1; // is_initialized
        provider
            .set_account(&wsol_mint, data, &TOKEN_PROGRAM_ID, 10_000_000)
            .expect("inject wsol_mint fixture");
    }
    wsol_mint
}

/// Bundles the per-pool parameters every trade helper below needs, so none
/// of them take more than `(ctx, user, amount)` -- naclac's `#[instruction_args]`
/// macro solves this same shape for on-chain instructions, but these are
/// plain test helpers, so the equivalent fix here is the same one Rust
/// itself recommends: group the arguments into a struct.
struct TradeCtx<'a> {
    provider: &'a NaclacProvider,
    global_pda: Address,
    bonding_curve_pda: Address,
    mint: Address,
    base_token_program: Address,
    bonding_curve_bump: u8,
    creator: Address,
    fee_recipient: Address,
    buyback_vault: Address,
    buyback_bump: u8,
}

/// Buys `amount` base tokens with native SOL against a freshly created pool
/// -- works against either `create`'s classic-Token pool or `create_v2`'s
/// Token-2022 pool via `ctx.base_token_program`, since `buy.rs`'s own
/// `token_program: Interface<TokenInterface>` accepts either.
fn buy(ctx: &TradeCtx, user: &Keypair, amount: u64) {
    let provider = ctx.provider;

    let (associated_bonding_curve_pda, associated_bonding_curve_bump) = Address::find_program_address(
        &[ctx.bonding_curve_pda.as_ref(), ctx.base_token_program.as_ref(), ctx.mint.as_ref()],
        &ASSOCIATED_TOKEN_PROGRAM_ID,
    );
    let (associated_user_pda, associated_user_bump) = Address::find_program_address(
        &[user.address().as_ref(), ctx.base_token_program.as_ref(), ctx.mint.as_ref()],
        &ASSOCIATED_TOKEN_PROGRAM_ID,
    );
    // Idempotent: `user` is the same reused trading wallet across every
    // `buy`/`buy_v2` call in `full_lifecycle`, so this ATA may already
    // exist by the time `buy_v2` runs (or vice versa on a rerun).
    if provider.get_account_data(&associated_user_pda).is_err() {
        create_ata_with_program(provider, &ctx.mint, &user.address(), &ctx.base_token_program)
            .expect("create user base ATA");
    }
    let (creator_vault_pda, creator_vault_bump) =
        Address::find_program_address(&[CREATOR_VAULT_SEED, ctx.creator.as_ref()], &PROGRAM_ID);
    let (global_volume_accumulator_pda, _) =
        Address::find_program_address(&[GLOBAL_VOLUME_ACCUMULATOR_SEED], &PROGRAM_ID);
    let (user_volume_accumulator_pda, user_volume_accumulator_bump) = Address::find_program_address(
        &[USER_VOLUME_ACCUMULATOR_SEED, user.address().as_ref()],
        &PROGRAM_ID,
    );
    let (fee_config_pda, fee_config_bump) =
        Address::find_program_address(&[FEE_CONFIG_SEED, PROGRAM_ID.as_ref()], &PUMP_FEES_PROGRAM_ID);
    let (pump_authority_pda, _) = Address::find_program_address(&[PUMP_AUTHORITY_SEED], &PROGRAM_ID);
    let (bonding_curve_v2_pda_addr, bonding_curve_v2_bump) =
        Address::find_program_address(&[BONDING_CURVE_V2_SEED, ctx.mint.as_ref()], &PROGRAM_ID);

    build_buy(
        provider,
        PROGRAM_ID,
        pump_client::BuyArgs {
            amount,
            max_sol_cost: 5_000_000_000,
            track_volume: Bool::from(false),
            bonding_curve_bump: ctx.bonding_curve_bump,
            associated_bonding_curve_bump,
            associated_user_bump,
            creator_vault_bump,
            user_volume_accumulator_bump,
            fee_config_bump,
            buyback_index: 0,
            buyback_vault_bump: ctx.buyback_bump,
            bonding_curve_v2_bump,
            ..Default::default()
        },
        BuyAccounts {
            global: ctx.global_pda,
            fee_recipient: ctx.fee_recipient,
            mint: ctx.mint,
            bonding_curve: ctx.bonding_curve_pda,
            user: user.address(),
            associated_bonding_curve: associated_bonding_curve_pda,
            associated_user: associated_user_pda,
            system_program: SYSTEM_PROGRAM_ID,
            token_program: ctx.base_token_program,
            creator_vault: creator_vault_pda,
            program: PROGRAM_ID,
            global_volume_accumulator: global_volume_accumulator_pda,
            user_volume_accumulator: user_volume_accumulator_pda,
            fee_config: fee_config_pda,
            fee_program: PUMP_FEES_PROGRAM_ID,
            bonding_curve_v2: bonding_curve_v2_pda_addr,
            buyback_fee_recipient: ctx.buyback_vault,
            pump_authority: pump_authority_pda,
        },
    )
    .signer(user)
    .log()
    .send_and_confirm()
    .expect("buy should succeed");
}

/// Sells `amount` base tokens back for native SOL -- mirrors `buy`'s own
/// reasoning for working against either pool via `ctx.base_token_program`.
/// Assumes `user` already holds a base-token ATA and a real balance (from a
/// preceding `buy` call with the same `user`).
fn sell(ctx: &TradeCtx, user: &Keypair, amount: u64) {
    let provider = ctx.provider;
    let (associated_bonding_curve_pda, associated_bonding_curve_bump) = Address::find_program_address(
        &[ctx.bonding_curve_pda.as_ref(), ctx.base_token_program.as_ref(), ctx.mint.as_ref()],
        &ASSOCIATED_TOKEN_PROGRAM_ID,
    );
    let (associated_user_pda, associated_user_bump) = Address::find_program_address(
        &[user.address().as_ref(), ctx.base_token_program.as_ref(), ctx.mint.as_ref()],
        &ASSOCIATED_TOKEN_PROGRAM_ID,
    );
    let (creator_vault_pda, creator_vault_bump) =
        Address::find_program_address(&[CREATOR_VAULT_SEED, ctx.creator.as_ref()], &PROGRAM_ID);
    let (user_volume_accumulator_pda, user_volume_accumulator_bump) = Address::find_program_address(
        &[USER_VOLUME_ACCUMULATOR_SEED, user.address().as_ref()],
        &PROGRAM_ID,
    );
    let (fee_config_pda, fee_config_bump) =
        Address::find_program_address(&[FEE_CONFIG_SEED, PROGRAM_ID.as_ref()], &PUMP_FEES_PROGRAM_ID);
    let (pump_authority_pda, _) = Address::find_program_address(&[PUMP_AUTHORITY_SEED], &PROGRAM_ID);
    let (bonding_curve_v2_pda_addr, bonding_curve_v2_bump) =
        Address::find_program_address(&[BONDING_CURVE_V2_SEED, ctx.mint.as_ref()], &PROGRAM_ID);

    build_sell(
        provider,
        PROGRAM_ID,
        pump_client::SellArgs {
            amount,
            min_sol_output: 1,
            bonding_curve_bump: ctx.bonding_curve_bump,
            associated_bonding_curve_bump,
            associated_user_bump,
            creator_vault_bump,
            user_volume_accumulator_bump,
            fee_config_bump,
            buyback_index: 0,
            buyback_vault_bump: ctx.buyback_bump,
            bonding_curve_v2_bump,
            ..Default::default()
        },
        SellAccounts {
            global: ctx.global_pda,
            fee_recipient: ctx.fee_recipient,
            mint: ctx.mint,
            bonding_curve: ctx.bonding_curve_pda,
            user: user.address(),
            associated_bonding_curve: associated_bonding_curve_pda,
            associated_user: associated_user_pda,
            token_program: ctx.base_token_program,
            system_program: SYSTEM_PROGRAM_ID,
            creator_vault: creator_vault_pda,
            program: PROGRAM_ID,
            user_volume_accumulator: user_volume_accumulator_pda,
            fee_config: fee_config_pda,
            fee_program: PUMP_FEES_PROGRAM_ID,
            bonding_curve_v2: bonding_curve_v2_pda_addr,
            buyback_fee_recipient: ctx.buyback_vault,
            pump_authority: pump_authority_pda,
        },
    )
    .signer(user)
    .log()
    .send_and_confirm()
    .expect("sell should succeed");
}

/// `buy_v2` generalizes the *quote* side to any SPL token (see this
/// project's own investigation of why `buy_v2` exists at all, alongside
/// `buy`) -- here `quote_mint = WSOL_MINT`, which real `buy_v2` handles via
/// the exact same native-lamport flow as classic `buy`, never touching a
/// quote-side WSOL ATA (confirmed in `buy_v2.rs`'s own module comment).
/// Requires `global_volume_accumulator` to already exist (`buy_v2.rs`'s own
/// account has no `init_if_needed`, unlike classic `buy`'s) -- `full_lifecycle`
/// only calls this after a real `buy` has already run at least once.
fn buy_v2(ctx: &TradeCtx, user: &Keypair, amount: u64) {
    let provider = ctx.provider;
    let wsol_mint = ensure_wsol_mint(provider);

    let (associated_base_bonding_curve_pda, associated_base_bonding_curve_bump) = Address::find_program_address(
        &[ctx.bonding_curve_pda.as_ref(), ctx.base_token_program.as_ref(), ctx.mint.as_ref()],
        &ASSOCIATED_TOKEN_PROGRAM_ID,
    );
    let (associated_quote_bonding_curve_pda, associated_quote_bonding_curve_bump) = Address::find_program_address(
        &[ctx.bonding_curve_pda.as_ref(), TOKEN_PROGRAM_ID.as_ref(), wsol_mint.as_ref()],
        &ASSOCIATED_TOKEN_PROGRAM_ID,
    );
    if provider.get_account_data(&associated_quote_bonding_curve_pda).is_err() {
        create_ata(provider, &wsol_mint, &ctx.bonding_curve_pda).expect("create bonding_curve quote ATA");
    }
    let (associated_base_user_pda, associated_base_user_bump) = Address::find_program_address(
        &[user.address().as_ref(), ctx.base_token_program.as_ref(), ctx.mint.as_ref()],
        &ASSOCIATED_TOKEN_PROGRAM_ID,
    );
    // Idempotent: `user` is the same reused trading wallet across every
    // `buy`/`buy_v2` call in `full_lifecycle`.
    if provider.get_account_data(&associated_base_user_pda).is_err() {
        create_ata_with_program(provider, &ctx.mint, &user.address(), &ctx.base_token_program)
            .expect("create user base ATA");
    }
    let (associated_quote_user_pda, associated_quote_user_bump) = Address::find_program_address(
        &[user.address().as_ref(), TOKEN_PROGRAM_ID.as_ref(), wsol_mint.as_ref()],
        &ASSOCIATED_TOKEN_PROGRAM_ID,
    );
    if provider.get_account_data(&associated_quote_user_pda).is_err() {
        create_ata(provider, &wsol_mint, &user.address()).expect("create user quote ATA");
    }
    let (creator_vault_pda, creator_vault_bump) =
        Address::find_program_address(&[CREATOR_VAULT_SEED, ctx.creator.as_ref()], &PROGRAM_ID);
    let (associated_creator_vault_pda, _associated_creator_vault_bump) = Address::find_program_address(
        &[creator_vault_pda.as_ref(), TOKEN_PROGRAM_ID.as_ref(), wsol_mint.as_ref()],
        &ASSOCIATED_TOKEN_PROGRAM_ID,
    );
    let (associated_quote_fee_recipient_pda, _associated_quote_fee_recipient_bump) = Address::find_program_address(
        &[ctx.fee_recipient.as_ref(), TOKEN_PROGRAM_ID.as_ref(), wsol_mint.as_ref()],
        &ASSOCIATED_TOKEN_PROGRAM_ID,
    );
    let (associated_quote_buyback_fee_recipient_pda, associated_quote_buyback_fee_recipient_bump) =
        Address::find_program_address(
            &[ctx.buyback_vault.as_ref(), TOKEN_PROGRAM_ID.as_ref(), wsol_mint.as_ref()],
            &ASSOCIATED_TOKEN_PROGRAM_ID,
        );
    if provider.get_account_data(&associated_quote_buyback_fee_recipient_pda).is_err() {
        create_ata(provider, &wsol_mint, &ctx.buyback_vault).expect("create buyback quote ATA");
    }
    let (global_volume_accumulator_pda, _) =
        Address::find_program_address(&[GLOBAL_VOLUME_ACCUMULATOR_SEED], &PROGRAM_ID);
    let (user_volume_accumulator_pda, user_volume_accumulator_bump) = Address::find_program_address(
        &[USER_VOLUME_ACCUMULATOR_SEED, user.address().as_ref()],
        &PROGRAM_ID,
    );
    let (associated_user_volume_accumulator_pda, _associated_user_volume_accumulator_bump) =
        Address::find_program_address(
            &[user_volume_accumulator_pda.as_ref(), TOKEN_PROGRAM_ID.as_ref(), wsol_mint.as_ref()],
            &ASSOCIATED_TOKEN_PROGRAM_ID,
        );
    let (fee_config_pda, fee_config_bump) =
        Address::find_program_address(&[FEE_CONFIG_SEED, PROGRAM_ID.as_ref()], &PUMP_FEES_PROGRAM_ID);
    let (pump_authority_pda, _) = Address::find_program_address(&[PUMP_AUTHORITY_SEED], &PROGRAM_ID);

    build_buy_v2(
        provider,
        PROGRAM_ID,
        BuyV2Args {
            amount,
            max_sol_cost: 5_000_000_000,
            bonding_curve_bump: ctx.bonding_curve_bump,
            associated_base_bonding_curve_bump,
            associated_quote_bonding_curve_bump,
            associated_base_user_bump,
            associated_quote_user_bump,
            creator_vault_bump,
            associated_quote_buyback_fee_recipient_bump,
            user_volume_accumulator_bump,
            fee_config_bump,
            buyback_index: 0,
            buyback_vault_bump: ctx.buyback_bump,
            ..Default::default()
        },
        BuyV2Accounts {
            global: ctx.global_pda,
            base_mint: ctx.mint,
            quote_mint: wsol_mint,
            base_token_program: ctx.base_token_program,
            quote_token_program: TOKEN_PROGRAM_ID,
            associated_token_program: ASSOCIATED_TOKEN_PROGRAM_ID,
            fee_recipient: ctx.fee_recipient,
            associated_quote_fee_recipient: associated_quote_fee_recipient_pda,
            buyback_fee_recipient: ctx.buyback_vault,
            associated_quote_buyback_fee_recipient: associated_quote_buyback_fee_recipient_pda,
            bonding_curve: ctx.bonding_curve_pda,
            associated_base_bonding_curve: associated_base_bonding_curve_pda,
            associated_quote_bonding_curve: associated_quote_bonding_curve_pda,
            user: user.address(),
            associated_base_user: associated_base_user_pda,
            associated_quote_user: associated_quote_user_pda,
            creator_vault: creator_vault_pda,
            associated_creator_vault: associated_creator_vault_pda,
            sharing_config: Address::default(),
            global_volume_accumulator: global_volume_accumulator_pda,
            user_volume_accumulator: user_volume_accumulator_pda,
            associated_user_volume_accumulator: associated_user_volume_accumulator_pda,
            program: PROGRAM_ID,
            fee_config: fee_config_pda,
            fee_program: PUMP_FEES_PROGRAM_ID,
            system_program: SYSTEM_PROGRAM_ID,
            pump_authority: pump_authority_pda,
        },
    )
    .signer(user)
    .log()
    .send_and_confirm()
    .expect("buy_v2 should succeed");
}

/// Mirrors `buy_v2`'s own reasoning; assumes `user`'s ATAs and every shared
/// (bonding_curve/buyback) quote ATA already exist from a preceding `buy_v2`
/// call with the same `user`, so this makes no `create_ata` calls itself.
fn sell_v2(ctx: &TradeCtx, user: &Keypair, amount: u64) {
    let provider = ctx.provider;
    let wsol_mint = wsol_mint_address();

    let (associated_base_bonding_curve_pda, associated_base_bonding_curve_bump) = Address::find_program_address(
        &[ctx.bonding_curve_pda.as_ref(), ctx.base_token_program.as_ref(), ctx.mint.as_ref()],
        &ASSOCIATED_TOKEN_PROGRAM_ID,
    );
    let (associated_quote_bonding_curve_pda, associated_quote_bonding_curve_bump) = Address::find_program_address(
        &[ctx.bonding_curve_pda.as_ref(), TOKEN_PROGRAM_ID.as_ref(), wsol_mint.as_ref()],
        &ASSOCIATED_TOKEN_PROGRAM_ID,
    );
    let (associated_base_user_pda, associated_base_user_bump) = Address::find_program_address(
        &[user.address().as_ref(), ctx.base_token_program.as_ref(), ctx.mint.as_ref()],
        &ASSOCIATED_TOKEN_PROGRAM_ID,
    );
    let (associated_quote_user_pda, associated_quote_user_bump) = Address::find_program_address(
        &[user.address().as_ref(), TOKEN_PROGRAM_ID.as_ref(), wsol_mint.as_ref()],
        &ASSOCIATED_TOKEN_PROGRAM_ID,
    );
    let (creator_vault_pda, creator_vault_bump) =
        Address::find_program_address(&[CREATOR_VAULT_SEED, ctx.creator.as_ref()], &PROGRAM_ID);
    let (associated_creator_vault_pda, _associated_creator_vault_bump) = Address::find_program_address(
        &[creator_vault_pda.as_ref(), TOKEN_PROGRAM_ID.as_ref(), wsol_mint.as_ref()],
        &ASSOCIATED_TOKEN_PROGRAM_ID,
    );
    let (associated_quote_fee_recipient_pda, _associated_quote_fee_recipient_bump) = Address::find_program_address(
        &[ctx.fee_recipient.as_ref(), TOKEN_PROGRAM_ID.as_ref(), wsol_mint.as_ref()],
        &ASSOCIATED_TOKEN_PROGRAM_ID,
    );
    let (associated_quote_buyback_fee_recipient_pda, associated_quote_buyback_fee_recipient_bump) =
        Address::find_program_address(
            &[ctx.buyback_vault.as_ref(), TOKEN_PROGRAM_ID.as_ref(), wsol_mint.as_ref()],
            &ASSOCIATED_TOKEN_PROGRAM_ID,
        );
    let (user_volume_accumulator_pda, user_volume_accumulator_bump) = Address::find_program_address(
        &[USER_VOLUME_ACCUMULATOR_SEED, user.address().as_ref()],
        &PROGRAM_ID,
    );
    let (associated_user_volume_accumulator_pda, _associated_user_volume_accumulator_bump) =
        Address::find_program_address(
            &[user_volume_accumulator_pda.as_ref(), TOKEN_PROGRAM_ID.as_ref(), wsol_mint.as_ref()],
            &ASSOCIATED_TOKEN_PROGRAM_ID,
        );
    let (fee_config_pda, fee_config_bump) =
        Address::find_program_address(&[FEE_CONFIG_SEED, PROGRAM_ID.as_ref()], &PUMP_FEES_PROGRAM_ID);
    let (pump_authority_pda, _) = Address::find_program_address(&[PUMP_AUTHORITY_SEED], &PROGRAM_ID);

    build_sell_v2(
        provider,
        PROGRAM_ID,
        SellV2Args {
            amount,
            min_sol_output: 0,
            bonding_curve_bump: ctx.bonding_curve_bump,
            associated_base_bonding_curve_bump,
            associated_quote_bonding_curve_bump,
            associated_base_user_bump,
            associated_quote_user_bump,
            creator_vault_bump,
            associated_quote_buyback_fee_recipient_bump,
            user_volume_accumulator_bump,
            fee_config_bump,
            buyback_index: 0,
            buyback_vault_bump: ctx.buyback_bump,
            ..Default::default()
        },
        SellV2Accounts {
            global: ctx.global_pda,
            base_mint: ctx.mint,
            quote_mint: wsol_mint,
            base_token_program: ctx.base_token_program,
            quote_token_program: TOKEN_PROGRAM_ID,
            associated_token_program: ASSOCIATED_TOKEN_PROGRAM_ID,
            fee_recipient: ctx.fee_recipient,
            associated_quote_fee_recipient: associated_quote_fee_recipient_pda,
            buyback_fee_recipient: ctx.buyback_vault,
            associated_quote_buyback_fee_recipient: associated_quote_buyback_fee_recipient_pda,
            bonding_curve: ctx.bonding_curve_pda,
            associated_base_bonding_curve: associated_base_bonding_curve_pda,
            associated_quote_bonding_curve: associated_quote_bonding_curve_pda,
            user: user.address(),
            associated_base_user: associated_base_user_pda,
            associated_quote_user: associated_quote_user_pda,
            creator_vault: creator_vault_pda,
            associated_creator_vault: associated_creator_vault_pda,
            sharing_config: Address::default(),
            user_volume_accumulator: user_volume_accumulator_pda,
            associated_user_volume_accumulator: associated_user_volume_accumulator_pda,
            program: PROGRAM_ID,
            fee_config: fee_config_pda,
            fee_program: PUMP_FEES_PROGRAM_ID,
            system_program: SYSTEM_PROGRAM_ID,
            pump_authority: pump_authority_pda,
        },
    )
    .signer(user)
    .log()
    .send_and_confirm()
    .expect("sell_v2 should succeed");
}

#[test]
fn full_lifecycle() {
    let provider = setup();
    let trader = if provider.cluster == RpcCluster::Litesvm {
        let trader = Keypair::new();
        provider.airdrop(&trader.address(), 5_000_000_000).expect("airdrop trader");
        trader
    } else {
        let trader = load_test_trader_keypair();
        assert!(
            provider.get_balance(&trader.address()).unwrap_or(0) > 0,
            "test trader wallet {} has no balance on this cluster -- fund it before running \
             pump_lifecycle_test.rs here",
            trader.address()
        );
        trader
    };

    let (global_pda, _bump) = get_global_pda(&PROGRAM_ID);
    ensure_extended(&provider, global_pda);

    // Also real, persistent, potentially-stale accounts (unlike
    // `bonding_curve`, freshly `init`ed on a brand-new mint every run, or
    // `sharing_config`, unused in this flow) -- `global_volume_accumulator`
    // is a singleton and `user_volume_accumulator` is keyed to the same
    // trader wallet reused across every run of this test.
    let (global_volume_accumulator_pda, _bump) = get_global_volume_accumulator_pda(&PROGRAM_ID);
    ensure_extended(&provider, global_volume_accumulator_pda);
    let (user_volume_accumulator_pda, _bump) =
        get_user_volume_accumulator_pda(&PROGRAM_ID, &trader.address());
    ensure_extended(&provider, user_volume_accumulator_pda);

    let global_pda = ensure_global(&provider);
    let fee_recipients = ensure_admin_setup(&provider, global_pda);
    let (buyback_vaults, buyback_bumps) = ensure_buyback_vaults(&provider, global_pda);
    ensure_reserved_fee_recipients(&provider, global_pda);
    ensure_fee_config_fixture(&provider);
    let global = fetch_global(&provider, &global_pda).expect("global should be readable");
    assert!(bool::from(global.initialized));
    assert_ne!(global.token_total_supply, 0, "admin setup should have configured a real token_total_supply");
    assert_ne!(global.buyback_fee_recipients, [Address::default(); 8], "buyback vaults should be registered");

    let creator = provider.payer.address();
    let fee_recipient = fee_recipients[0];
    let buyback_vault = buyback_vaults[0];
    let buyback_bump = buyback_bumps[0];
    let buy_amount = 10_000_000_000_000u64;
    let sell_amount = buy_amount / 2;

    let (bonding_curve_v1_pda, mint_v1) = create_pool_v1(&provider, global_pda);
    let (_, bonding_curve_v1_bump) = get_bonding_curve_pda(&PROGRAM_ID, &mint_v1.address());
    let pool_v1 = fetch_bonding_curve(&provider, &bonding_curve_v1_pda).expect("pool v1 should be readable");
    assert_eq!(pool_v1.token_total_supply, global.token_total_supply);
    assert_eq!(pool_v1.creator, Address::default(), "create_pool_v1 should leave creator unset");
    println!("pool v1 mint: {}", mint_v1.address());

    // Real-lifecycle placement: a token whose creator wasn't set at creation
    // time gets backfilled via `set_creator` before it starts trading (the
    // real-mainnet-observed usage pattern -- see
    // `docs/plan/bonding-curve-04-open-questions.md`). `create`'s own
    // Metaplex metadata CPI always encodes `creators: None`
    // (`metadata_cpi.rs`), so `set_creator`'s real, confirmed logic falls
    // back to writing the `creator` arg directly.
    let (metadata_v1_pda, metadata_v1_bump) = get_metadata_pda(&mint_v1.address());
    build_set_creator(
        &provider,
        PROGRAM_ID,
        creator,
        metadata_v1_bump,
        bonding_curve_v1_bump,
        SetCreatorAccounts {
            set_creator_authority: provider.payer.address(),
            global: global_pda,
            mint: mint_v1.address(),
            metadata: metadata_v1_pda,
            bonding_curve: bonding_curve_v1_pda,
        },
    )
    .log()
    .send_and_confirm()
    .expect("set_creator should succeed");
    let pool_v1 = fetch_bonding_curve(&provider, &bonding_curve_v1_pda).expect("pool v1 should be readable");
    assert_eq!(pool_v1.creator, creator, "set_creator should have backfilled the creator");

    let ctx_v1 = TradeCtx {
        provider: &provider,
        global_pda,
        bonding_curve_pda: bonding_curve_v1_pda,
        mint: mint_v1.address(),
        base_token_program: TOKEN_PROGRAM_ID,
        bonding_curve_bump: bonding_curve_v1_bump,
        creator,
        fee_recipient,
        buyback_vault,
        buyback_bump,
    };

    // Classic `buy` must run at least once, anywhere, before any `buy_v2`
    // call -- `buy_v2.rs`'s own `global_volume_accumulator` account has no
    // `init_if_needed`, unlike classic `buy`'s.
    buy(&ctx_v1, &trader, buy_amount);
    sell(&ctx_v1, &trader, sell_amount);

    buy_v2(&ctx_v1, &trader, buy_amount);
    sell_v2(&ctx_v1, &trader, sell_amount);

    let (bonding_curve_v2_pda, mint_v2) = create_pool_v2(&provider, global_pda);
    let (_, bonding_curve_v2_bump) = get_bonding_curve_pda(&PROGRAM_ID, &mint_v2.address());
    let pool_v2 = fetch_bonding_curve(&provider, &bonding_curve_v2_pda).expect("pool v2 should be readable");
    assert_eq!(pool_v2.token_total_supply, global.token_total_supply);
    println!("pool v2 mint: {}", mint_v2.address());

    let ctx_v2 = TradeCtx {
        provider: &provider,
        global_pda,
        bonding_curve_pda: bonding_curve_v2_pda,
        mint: mint_v2.address(),
        base_token_program: TOKEN_2022_PROGRAM_ID,
        bonding_curve_bump: bonding_curve_v2_bump,
        creator,
        fee_recipient,
        buyback_vault,
        buyback_bump,
    };

    buy(&ctx_v2, &trader, buy_amount);
    sell(&ctx_v2, &trader, sell_amount);

    buy_v2(&ctx_v2, &trader, buy_amount);
    sell_v2(&ctx_v2, &trader, sell_amount);

    // Quote-mint whitelist: `add_quote_mint`/`remove_quote_mint`/
    // `set_virtual_quote_reserves`, ported from `pump_test.rs`'s
    // `add_quote_mint_enables_and_remove_quote_mint_disables_create_v2_with_that_mint`
    // (already proven there) -- same self-contained add-use-remove cycle, so
    // `Global.whitelisted_quote_mints` is left exactly as this run found it,
    // safe to repeat on every run against real, persistent devnet state.
    // Mimic-SOL: a freshly minted, 9-decimal classic SPL Token mint, not a
    // real cloned mainnet mint -- `add_quote_mint`/`create_v2` only need a
    // real, valid mint account, not any specific one.
    let quote_mint_kp = Keypair::new();
    create_mint(&provider, &quote_mint_kp, &provider.payer.address(), 9)
        .expect("create mimic-SOL quote mint should succeed");
    let quote_mint = quote_mint_kp.address();

    build_add_quote_mint(
        &provider,
        PROGRAM_ID,
        quote_mint,
        AddQuoteMintAccounts { global: global_pda, authority: provider.payer.address() },
    )
    .log()
    .send_and_confirm()
    .expect("add_quote_mint should succeed");

    // Real value, not a guess: `Global.initial_virtual_quote_reserves`
    // fetched directly off the real mainnet `Global` account, confirmed via
    // `reference/fee-tier-probe/src/bin/probe64.rs`'s own printed output.
    const REAL_INITIAL_VIRTUAL_QUOTE_RESERVES: u64 = 4_292_000_000;
    build_set_virtual_quote_reserves(
        &provider,
        PROGRAM_ID,
        REAL_INITIAL_VIRTUAL_QUOTE_RESERVES,
        SetVirtualQuoteReservesAccounts { global: global_pda, authority: provider.payer.address() },
    )
    .log()
    .send_and_confirm()
    .expect("set_virtual_quote_reserves should succeed");

    let (result, bonding_curve_quote_pda, _mint) =
        create_pool_v2_with_quote_mint(&provider, global_pda, Some(quote_mint));
    result.expect("create_v2 with a whitelisted quote mint should succeed");

    let bonding_curve_quote =
        fetch_bonding_curve(&provider, &bonding_curve_quote_pda).expect("bonding_curve should be readable");
    assert_eq!(bonding_curve_quote.quote_mint, quote_mint, "bonding_curve should be paired with the whitelisted quote mint");
    assert_eq!(
        bonding_curve_quote.virtual_quote_reserves, REAL_INITIAL_VIRTUAL_QUOTE_RESERVES,
        "non-SOL-paired curve should seed virtual_quote_reserves from Global.initial_virtual_quote_reserves"
    );

    build_remove_quote_mint(
        &provider,
        PROGRAM_ID,
        quote_mint,
        RemoveQuoteMintAccounts { global: global_pda, authority: provider.payer.address() },
    )
    .log()
    .send_and_confirm()
    .expect("remove_quote_mint should succeed");

    let (result, _bonding_curve_pda, _mint) =
        create_pool_v2_with_quote_mint(&provider, global_pda, Some(quote_mint));
    // `PumpError::QuoteMintNotWhitelisted` is enum index 5 -> 6000 + 5 = 6005.
    assert_custom_code(result, 6005);

    // Real, non-SOL-quote graduation + migration -- the actual point of
    // building `add_quote_mint`/`set_virtual_quote_reserves`/`create_config`
    // in the first place: proves the whole chain works on real devnet
    // without spending real SOL on a real SOL-paired graduation (~85
    // SOL/pool). The base coin/mint is fresh every run (see module
    // comment); the mimic-SOL quote mint itself is the one persistent
    // piece, whitelisted once and reused indefinitely.
    let mimic_sol_mint = ensure_mimic_sol_mint(&provider);
    ensure_quote_mint_whitelisted(&provider, global_pda, mimic_sol_mint);
    ensure_virtual_quote_reserves(&provider, global_pda);

    let (result, bonding_curve_quote_pda, mint_quote) =
        create_pool_v2_with_quote_mint(&provider, global_pda, Some(mimic_sol_mint));
    result.expect("create_v2 with the whitelisted mimic-SOL quote mint should succeed");
    let (_, bonding_curve_quote_bump) = get_bonding_curve_pda(&PROGRAM_ID, &mint_quote.address());
    println!("quote-paired pool mint: {}", mint_quote.address());

    let (associated_base_bonding_curve_quote_pda, associated_base_bonding_curve_quote_bump) = Address::find_program_address(
        &[bonding_curve_quote_pda.as_ref(), TOKEN_2022_PROGRAM_ID.as_ref(), mint_quote.address().as_ref()],
        &ASSOCIATED_TOKEN_PROGRAM_ID,
    );
    let (associated_quote_bonding_curve_quote_pda, associated_quote_bonding_curve_quote_bump) = Address::find_program_address(
        &[bonding_curve_quote_pda.as_ref(), TOKEN_PROGRAM_ID.as_ref(), mimic_sol_mint.as_ref()],
        &ASSOCIATED_TOKEN_PROGRAM_ID,
    );
    let (associated_base_trader_pda, associated_base_trader_bump) = Address::find_program_address(
        &[trader.address().as_ref(), TOKEN_2022_PROGRAM_ID.as_ref(), mint_quote.address().as_ref()],
        &ASSOCIATED_TOKEN_PROGRAM_ID,
    );
    if provider.get_account_data(&associated_base_trader_pda).is_err() {
        create_ata_with_program(&provider, &mint_quote.address(), &trader.address(), &TOKEN_2022_PROGRAM_ID)
            .expect("create trader base ATA");
    }
    let (associated_quote_trader_pda, associated_quote_trader_bump) = Address::find_program_address(
        &[trader.address().as_ref(), TOKEN_PROGRAM_ID.as_ref(), mimic_sol_mint.as_ref()],
        &ASSOCIATED_TOKEN_PROGRAM_ID,
    );
    if provider.get_account_data(&associated_quote_trader_pda).is_err() {
        create_ata(&provider, &mimic_sol_mint, &trader.address()).expect("create trader quote ATA");
    }
    // Real graduation cost for this curve (~12.4e9 mimic-SOL base units, per
    // `pump_test.rs`'s own hand-derived figure for the identical reserve
    // constants) plus fee headroom -- minted generously above that, not
    // tightly, since this is a test fixture, not a real economic
    // constraint. Additive across runs (never spent elsewhere), harmless.
    mint_to(&provider, &mimic_sol_mint, &associated_quote_trader_pda, &provider.payer, 20_000_000_000)
        .expect("mint mimic-SOL supply to trader should succeed");

    let (creator_vault_quote_pda, creator_vault_quote_bump) =
        Address::find_program_address(&[CREATOR_VAULT_SEED, creator.as_ref()], &PROGRAM_ID);
    let (associated_creator_vault_quote_pda, _associated_creator_vault_quote_bump) = Address::find_program_address(
        &[creator_vault_quote_pda.as_ref(), TOKEN_PROGRAM_ID.as_ref(), mimic_sol_mint.as_ref()],
        &ASSOCIATED_TOKEN_PROGRAM_ID,
    );
    let (associated_quote_fee_recipient_quote_pda, _associated_quote_fee_recipient_quote_bump) = Address::find_program_address(
        &[fee_recipient.as_ref(), TOKEN_PROGRAM_ID.as_ref(), mimic_sol_mint.as_ref()],
        &ASSOCIATED_TOKEN_PROGRAM_ID,
    );
    let (associated_quote_buyback_fee_recipient_quote_pda, associated_quote_buyback_fee_recipient_quote_bump) =
        Address::find_program_address(
            &[buyback_vault.as_ref(), TOKEN_PROGRAM_ID.as_ref(), mimic_sol_mint.as_ref()],
            &ASSOCIATED_TOKEN_PROGRAM_ID,
        );
    if provider.get_account_data(&associated_quote_buyback_fee_recipient_quote_pda).is_err() {
        create_ata(&provider, &mimic_sol_mint, &buyback_vault).expect("create buyback quote ATA");
    }
    let (global_volume_accumulator_pda, _) = get_global_volume_accumulator_pda(&PROGRAM_ID);
    let (user_volume_accumulator_quote_pda, user_volume_accumulator_quote_bump) =
        get_user_volume_accumulator_pda(&PROGRAM_ID, &trader.address());
    let (associated_user_volume_accumulator_quote_pda, _associated_user_volume_accumulator_quote_bump) =
        Address::find_program_address(
            &[user_volume_accumulator_quote_pda.as_ref(), TOKEN_PROGRAM_ID.as_ref(), mimic_sol_mint.as_ref()],
            &ASSOCIATED_TOKEN_PROGRAM_ID,
        );
    let (fee_config_pda, fee_config_bump) =
        Address::find_program_address(&[FEE_CONFIG_SEED, PROGRAM_ID.as_ref()], &PUMP_FEES_PROGRAM_ID);
    let (pump_authority_pda, _) = Address::find_program_address(&[PUMP_AUTHORITY_SEED], &PROGRAM_ID);

    // Same real graduation trigger used throughout this file: buy the
    // curve's entire `real_token_reserves` in one `buy_v2` trade.
    let full_amount = 793_100_000_000_000u64;
    build_buy_v2(
        &provider,
        PROGRAM_ID,
        BuyV2Args {
            amount: full_amount,
            max_sol_cost: 20_000_000_000,
            bonding_curve_bump: bonding_curve_quote_bump,
            associated_base_bonding_curve_bump: associated_base_bonding_curve_quote_bump,
            associated_quote_bonding_curve_bump: associated_quote_bonding_curve_quote_bump,
            associated_base_user_bump: associated_base_trader_bump,
            associated_quote_user_bump: associated_quote_trader_bump,
            creator_vault_bump: creator_vault_quote_bump,
            associated_quote_buyback_fee_recipient_bump: associated_quote_buyback_fee_recipient_quote_bump,
            user_volume_accumulator_bump: user_volume_accumulator_quote_bump,
            fee_config_bump,
            buyback_index: 0,
            buyback_vault_bump: buyback_bump,
            ..Default::default()
        },
        BuyV2Accounts {
            global: global_pda,
            base_mint: mint_quote.address(),
            quote_mint: mimic_sol_mint,
            base_token_program: TOKEN_2022_PROGRAM_ID,
            quote_token_program: TOKEN_PROGRAM_ID,
            associated_token_program: ASSOCIATED_TOKEN_PROGRAM_ID,
            fee_recipient,
            associated_quote_fee_recipient: associated_quote_fee_recipient_quote_pda,
            buyback_fee_recipient: buyback_vault,
            associated_quote_buyback_fee_recipient: associated_quote_buyback_fee_recipient_quote_pda,
            bonding_curve: bonding_curve_quote_pda,
            associated_base_bonding_curve: associated_base_bonding_curve_quote_pda,
            associated_quote_bonding_curve: associated_quote_bonding_curve_quote_pda,
            user: trader.address(),
            associated_base_user: associated_base_trader_pda,
            associated_quote_user: associated_quote_trader_pda,
            creator_vault: creator_vault_quote_pda,
            associated_creator_vault: associated_creator_vault_quote_pda,
            sharing_config: Address::default(),
            global_volume_accumulator: global_volume_accumulator_pda,
            user_volume_accumulator: user_volume_accumulator_quote_pda,
            associated_user_volume_accumulator: associated_user_volume_accumulator_quote_pda,
            program: PROGRAM_ID,
            fee_config: fee_config_pda,
            fee_program: PUMP_FEES_PROGRAM_ID,
            system_program: SYSTEM_PROGRAM_ID,
            pump_authority: pump_authority_pda,
        },
    )
    .signer(&trader)
    .log()
    .send_and_confirm()
    .expect("graduating buy_v2 against the mimic-SOL quote mint should succeed");

    let graduated_quote = fetch_bonding_curve(&provider, &bonding_curve_quote_pda)
        .expect("bonding_curve should be readable");
    assert!(
        bool::from(graduated_quote.complete),
        "bonding curve should be complete after buying out its full reserves"
    );
    assert_eq!(graduated_quote.real_token_reserves, 0);

    // `migrate_v2` unconditionally CPIs into `init_boost`, which needs real
    // `pump_amm::GlobalConfig` to already exist with `boost_enabled = true`.
    // On a real cluster this is created for real via
    // `pump_amm_lifecycle_test.rs`'s own `ensure_global_config` (must be run
    // at least once against this same cluster before this will succeed). On
    // litesvm there's no shared state across separate test binaries/VMs, so
    // this file bootstraps its own `GlobalConfig` here instead.
    let (amm_global_config_pda, amm_global_config_bump) =
        Address::find_program_address(&[GLOBAL_CONFIG_SEED], &PUMP_AMM_PROGRAM_ID);

    if provider.cluster == RpcCluster::Litesvm {
        let admin = load_test_admin_keypair();
        provider.airdrop(&admin.address(), 10_000_000_000).unwrap();

        build_create_config(
            &provider,
            PUMP_AMM_PROGRAM_ID,
            CreateConfigAccounts {
                admin: admin.address(),
                global_config: amm_global_config_pda,
                system_program: SYSTEM_PROGRAM_ID,
            },
        )
        .signer(&admin)
        .log()
        .send_and_confirm()
        .expect("litesvm: pump_amm create_config should succeed");

        build_toggle_boost(
            &provider,
            PUMP_AMM_PROGRAM_ID,
            Bool::from(true),
            ToggleBoostAccounts { admin: admin.address(), global_config: amm_global_config_pda },
        )
        .signer(&admin)
        .log()
        .send_and_confirm()
        .expect("litesvm: pump_amm toggle_boost should succeed");
    }

    let (pool_authority_pda, pool_authority_bump) =
        Address::find_program_address(&[POOL_AUTHORITY_SEED, mint_quote.address().as_ref()], &PROGRAM_ID);
    let (pool_pda, pool_bump) = Address::find_program_address(
        &[POOL_SEED, &0u16.to_le_bytes(), pool_authority_pda.as_ref(), mint_quote.address().as_ref(), mimic_sol_mint.as_ref()],
        &PUMP_AMM_PROGRAM_ID,
    );
    let (pool_authority_mint_account_pda, pool_authority_mint_account_bump) = Address::find_program_address(
        &[pool_authority_pda.as_ref(), TOKEN_2022_PROGRAM_ID.as_ref(), mint_quote.address().as_ref()],
        &ASSOCIATED_TOKEN_PROGRAM_ID,
    );
    let (pool_authority_quote_account_pda, pool_authority_quote_account_bump) = Address::find_program_address(
        &[pool_authority_pda.as_ref(), TOKEN_PROGRAM_ID.as_ref(), mimic_sol_mint.as_ref()],
        &ASSOCIATED_TOKEN_PROGRAM_ID,
    );
    let (lp_mint_pda, lp_mint_bump) =
        Address::find_program_address(&[POOL_LP_MINT_SEED, pool_pda.as_ref()], &PUMP_AMM_PROGRAM_ID);
    let (user_pool_token_account_pda, user_pool_token_account_bump) = Address::find_program_address(
        &[pool_authority_pda.as_ref(), TOKEN_2022_PROGRAM_ID.as_ref(), lp_mint_pda.as_ref()],
        &ASSOCIATED_TOKEN_PROGRAM_ID,
    );
    let (pool_base_token_account_pda, pool_base_token_account_bump) = Address::find_program_address(
        &[pool_pda.as_ref(), TOKEN_2022_PROGRAM_ID.as_ref(), mint_quote.address().as_ref()],
        &ASSOCIATED_TOKEN_PROGRAM_ID,
    );
    let (pool_quote_token_account_pda, pool_quote_token_account_bump) = Address::find_program_address(
        &[pool_pda.as_ref(), TOKEN_PROGRAM_ID.as_ref(), mimic_sol_mint.as_ref()],
        &ASSOCIATED_TOKEN_PROGRAM_ID,
    );
    let (boost_vault_authority_pda, boost_vault_authority_bump) =
        Address::find_program_address(&[BOOST_VAULT_SEED, pool_pda.as_ref()], &PUMP_AMM_PROGRAM_ID);
    let (boost_vault_pda, boost_vault_bump) = Address::find_program_address(
        &[boost_vault_authority_pda.as_ref(), TOKEN_PROGRAM_ID.as_ref(), mimic_sol_mint.as_ref()],
        &ASSOCIATED_TOKEN_PROGRAM_ID,
    );

    build_migrate_v2(
        &provider,
        PROGRAM_ID,
        MigrateV2Args {
            bonding_curve_bump: bonding_curve_quote_bump,
            associated_base_bonding_curve_bump: associated_base_bonding_curve_quote_bump,
            associated_quote_bonding_curve_bump: associated_quote_bonding_curve_quote_bump,
            pool_authority_bump,
            pool_authority_mint_account_bump,
            pool_authority_quote_account_bump,
            amm_global_config_bump,
            pool_bump,
            lp_mint_bump,
            user_pool_token_account_bump,
            pool_base_token_account_bump,
            pool_quote_token_account_bump,
            boost_vault_authority_bump,
            boost_vault_bump,
            ..Default::default()
        },
        MigrateV2Accounts {
            global: global_pda,
            withdraw_authority: provider.payer.address(),
            base_mint: mint_quote.address(),
            quote_mint: mimic_sol_mint,
            bonding_curve: bonding_curve_quote_pda,
            associated_base_bonding_curve: associated_base_bonding_curve_quote_pda,
            associated_quote_bonding_curve: associated_quote_bonding_curve_quote_pda,
            user: trader.address(),
            system_program: SYSTEM_PROGRAM_ID,
            base_token_program: TOKEN_2022_PROGRAM_ID,
            quote_token_program: TOKEN_PROGRAM_ID,
            token_2022_program: TOKEN_2022_PROGRAM_ID,
            associated_token_program: ASSOCIATED_TOKEN_PROGRAM_ID,
            pump_amm: PUMP_AMM_PROGRAM_ID,
            rent: RENT_SYSVAR_ID,
            pool_authority: pool_authority_pda,
            pool: pool_pda,
            pool_authority_mint_account: pool_authority_mint_account_pda,
            pool_authority_quote_account: pool_authority_quote_account_pda,
            amm_global_config: amm_global_config_pda,
            lp_mint: lp_mint_pda,
            user_pool_token_account: user_pool_token_account_pda,
            pool_base_token_account: pool_base_token_account_pda,
            pool_quote_token_account: pool_quote_token_account_pda,
            boost_vault_authority: boost_vault_authority_pda,
            boost_vault: boost_vault_pda,
        },
    )
    .signer(&trader)
    .log()
    .send_and_confirm()
    .expect("migrate_v2 against the mimic-SOL quote mint should succeed");

    let migrated_quote = fetch_bonding_curve(&provider, &bonding_curve_quote_pda)
        .expect("bonding_curve should still be readable after migrate_v2");
    assert_eq!(migrated_quote.real_token_reserves, 0);
    assert_eq!(migrated_quote.real_quote_reserves, 0);
    println!("quote-paired pool migrated: pool={pool_pda}");
}
