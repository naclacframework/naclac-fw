//! Sequential, idempotent, cross-cluster lifecycle test for the pump_fees
//! program -- the pump_fees counterpart to `pump-bonding-curve`'s own
//! `pump_lifecycle_test.rs`. Creates the real on-chain state
//! (`FeeProgramGlobal`, `FeeConfig`, the 8 buyback vaults) that
//! `pump_lifecycle_test.rs`'s `update_buyback_config` step only ever
//! registers, never creates -- see that file's own doc comment for the
//! reasoning behind the split.
//!
//! `initialize_fee_program_global` requires `authority.address() ==
//! pump_global.authority`, so this file's `provider.payer` must be the same
//! wallet `pump_lifecycle_test.rs` used to run `pump::initialize` on the
//! same cluster.
//!
//! `initialize_fee_config`'s golden path requires a signer matching the
//! real program's hardcoded `ADMIN_PUBKEY` -- the private half of that key
//! isn't held by anyone outside this project, so this loads the test-only
//! keypair at `tests/wallets/test_admin.json` (pinned to that exact pubkey,
//! same as `pump_fees_test.rs`'s own `load_test_admin_keypair`) instead of
//! `provider.payer`.
//!
//! Cluster selection: edit the `CLUSTER` constant below, same as
//! `pump_lifecycle_test.rs`.

use naclac_client::*;
use pump_client::{
    fetch_global, get_global_pda,
    instructions::{build_initialize, InitializeAccounts},
};
use pump_fees_client::{
    fetch_fee_config, fetch_fee_program_global, get_fee_config_pda, get_fee_program_global_pda,
    instructions::{
        build_initialize_buyback, build_initialize_fee_config, build_initialize_fee_program_global,
        build_upsert_fee_tiers, build_upsert_stable_fee_tiers, InitializeBuybackAccounts,
        InitializeFeeConfigAccounts, InitializeFeeProgramGlobalAccounts, UpsertFeeTiersAccounts,
        UpsertStableFeeTiersAccounts,
    },
    Fees, FeeTier, BUYBACK_VAULT_SEED, PROGRAM_ID, PUMP_PROGRAM_ID,
};
use std::path::PathBuf;

/// Change this to switch clusters -- "litesvm", "localnet", "devnet",
/// "mainnet", or a raw RPC URL (see `NaclacProvider::new`).
const CLUSTER: &str = "devnet";

fn setup() -> NaclacProvider {
    let payer = load_node_wallet().expect("Failed to load local Solana keypair");
    let provider = NaclacProvider::new(CLUSTER, payer);

    if provider.cluster == RpcCluster::Litesvm {
        let mut own_workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        own_workspace_root.pop(); // programs
        own_workspace_root.pop(); // pump-fees workspace root
        let pump_fees_so = resolve_cargo_target_dir(&own_workspace_root).join("deploy/pump_fees.so");
        provider
            .add_program(&PROGRAM_ID, pump_fees_so.to_str().unwrap())
            .expect("Failed to load pump_fees.so");

        let mut pump_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        pump_root.pop(); // programs
        pump_root.pop(); // pump-fees workspace root
        pump_root.pop(); // pump-fun-program
        pump_root.push("pump-bonding-curve");
        let pump_so = resolve_cargo_target_dir(&pump_root).join("deploy/pump.so");
        provider
            .add_program(&PUMP_PROGRAM_ID, pump_so.to_str().unwrap())
            .expect("Failed to load pump.so");
    }

    provider
}

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

/// Idempotent, mirrors `pump_lifecycle_test.rs`'s own `ensure_global` --
/// `pump`'s real `Global` must already exist before
/// `initialize_fee_program_global` can read its `authority`.
fn ensure_pump_global(provider: &NaclacProvider) -> Address {
    let (global_pda, _) = get_global_pda(&PUMP_PROGRAM_ID);
    if fetch_global(provider, &global_pda).is_ok() {
        return global_pda;
    }

    build_initialize(
        provider,
        PUMP_PROGRAM_ID,
        InitializeAccounts {
            user: provider.payer.address(),
            global: global_pda,
            system_program: SYSTEM_PROGRAM_ID,
        },
    )
    .log()
    .send_and_confirm()
    .expect("pump::initialize should succeed");

    global_pda
}

/// Idempotent: real `initialize_fee_program_global` can only succeed once
/// (`fee_program_global` is `init`-only).
fn ensure_fee_program_global(provider: &NaclacProvider, pump_global_pda: Address) -> Address {
    let (fee_program_global_pda, _) = get_fee_program_global_pda(&PROGRAM_ID);
    if fetch_fee_program_global(provider, &fee_program_global_pda).is_ok() {
        return fee_program_global_pda;
    }

    let payer_address = provider.payer.address();
    build_initialize_fee_program_global(
        provider,
        PROGRAM_ID,
        payer_address,
        0,
        0,
        InitializeFeeProgramGlobalAccounts {
            authority: payer_address,
            pump_global: pump_global_pda,
            fee_program_global: fee_program_global_pda,
            system_program: SYSTEM_PROGRAM_ID,
        },
    )
    .log()
    .send_and_confirm()
    .expect("initialize_fee_program_global should succeed");

    fee_program_global_pda
}

/// Idempotent: real `initialize_fee_config` can only succeed once per
/// `config_program_id` (`fee_config` is `init`-only). Must sign with the
/// test-only admin keypair pinned to the real program's hardcoded
/// `ADMIN_PUBKEY` -- `provider.payer` cannot satisfy this constraint.
/// On a real cluster, funding that admin wallet is a human's job (same
/// reasoning as `pump_lifecycle_test.rs`'s own doc comment about
/// `provider.payer`'s funding story) -- this only asserts it already has a
/// balance rather than airdropping into it.
///
/// Also idempotently ensures `fee_tiers` AND `stable_fee_tiers` are both
/// non-empty via `upsert_fee_tiers`/`upsert_stable_fee_tiers` --
/// `buy`/`sell`/`buy_v2`/`sell_v2` all unconditionally CPI into
/// `pump_fees::get_fees` with `is_pump_pool = true`, which errors
/// `NoFeeTiers` against an empty table (confirmed via `get_fees.rs`/
/// `calculate_fee_tier`'s real source) -- `is_new_quote_mint` (true for any
/// non-SOL-quote `buy_v2`/`sell_v2` trade, `fees-04-fee-math-and-formulas.md`
/// #5, confirmed against real bytecode via `probe5.rs`) selects
/// `stable_fee_tiers` instead of `fee_tiers`, so a non-SOL-quote trade would
/// hit `NoFeeTiers` even with `fee_tiers` populated, until this also
/// populates `stable_fee_tiers`. A freshly `initialize_fee_config`'d account
/// has zero tiers in both tables, so no trade of either kind could ever
/// succeed without this. Both use `lp_fee_bps: 0, protocol_fee_bps: 95,
/// creator_fee_bps: 30` -- pump.fun's own real bonding-curve fee schedule.
/// `fee_tiers`' value is read directly off a real mainnet `buy` transaction's
/// `GetFees` CPI return data (tx
/// `16K6hbphM91syDQCBatvrq2LzqjBEsTGUgrZT5Fiu6ebj3VF3rwJH6YApjg6asesHXN3CoXntp8qbmH23WQbSH5`);
/// `stable_fee_tiers`' value matches it exactly, confirmed real via
/// `reference/fee-tier-probe/src/bin/probe5.rs` (real mainnet `fee_config`
/// bytes: `stable_fee_tiers[0]` currently equals `fee_tiers[0]`, `{0, 95,
/// 30}` in both) -- not assumed, not a placeholder.
fn ensure_fee_config(provider: &NaclacProvider) -> Address {
    let (fee_config_pda, fee_config_bump) = get_fee_config_pda(&PROGRAM_ID, &PUMP_PROGRAM_ID);
    let admin = load_test_admin_keypair();

    if let Ok(fee_config) = fetch_fee_config(provider, &fee_config_pda) {
        if fee_config.fee_tiers_len > 0 && fee_config.stable_fee_tiers_len > 0 {
            return fee_config_pda;
        }
    } else {
        if provider.cluster == RpcCluster::Litesvm {
            provider.airdrop(&admin.address(), 10_000_000_000).unwrap();
        } else {
            assert!(
                provider.get_balance(&admin.address()).unwrap_or(0) > 0,
                "test admin wallet {} has no balance on this cluster -- fund it before running \
                 pump_fees_lifecycle_test.rs here",
                admin.address()
            );
        }

        build_initialize_fee_config(
            provider,
            PROGRAM_ID,
            fee_config_bump,
            InitializeFeeConfigAccounts {
                admin: admin.address(),
                config_program_id: PUMP_PROGRAM_ID,
                fee_config: fee_config_pda,
                system_program: SYSTEM_PROGRAM_ID,
            },
        )
        .signer(&admin)
        .log()
        .send_and_confirm()
        .expect("initialize_fee_config should succeed for the real ADMIN_PUBKEY signer");
    }

    build_upsert_fee_tiers(
        provider,
        PROGRAM_ID,
        vec![FeeTier {
            market_cap_lamports_threshold: 0,
            fees: Fees { lp_fee_bps: 0, protocol_fee_bps: 95, creator_fee_bps: 30 },
        }],
        0,
        UpsertFeeTiersAccounts {
            admin: admin.address(),
            config_program_id: PUMP_PROGRAM_ID,
            fee_config: fee_config_pda,
        },
    )
    .signer(&admin)
    .log()
    .send_and_confirm()
    .expect("upsert_fee_tiers should succeed");

    build_upsert_stable_fee_tiers(
        provider,
        PROGRAM_ID,
        vec![FeeTier {
            market_cap_lamports_threshold: 0,
            fees: Fees { lp_fee_bps: 0, protocol_fee_bps: 95, creator_fee_bps: 30 },
        }],
        0,
        UpsertStableFeeTiersAccounts {
            admin: admin.address(),
            config_program_id: PUMP_PROGRAM_ID,
            fee_config: fee_config_pda,
        },
    )
    .signer(&admin)
    .log()
    .send_and_confirm()
    .expect("upsert_stable_fee_tiers should succeed");

    fee_config_pda
}

/// Creates `pump_fees`'s 8 real buyback vaults via `initialize_buyback` --
/// the actual creation step `pump_lifecycle_test.rs`'s own
/// `ensure_buyback_vaults` deliberately does not perform (that file only
/// ever registers already-existing vaults with `pump`'s `Global`).
/// Idempotent per-index: `buyback_vault` is `init`-only, so an already
/// existing vault at a given index is left untouched. All 8 share one
/// throwaway test mint -- `initialize_buyback.rs`'s `mint: Account<Mint>`
/// has no protocol-level constraint pinning it to a specific quote mint.
fn ensure_buyback_vaults(provider: &NaclacProvider) -> [Address; 8] {
    let mut vaults = [Address::default(); 8];
    let mut missing = [false; 8];
    let mut any_missing = false;
    for i in 0..8u8 {
        let (buyback_vault_pda, _) = Address::find_program_address(&[&BUYBACK_VAULT_SEED, &[i]], &PROGRAM_ID);
        vaults[i as usize] = buyback_vault_pda;
        let is_missing = provider.get_account_data(&buyback_vault_pda).is_err();
        missing[i as usize] = is_missing;
        any_missing |= is_missing;
    }

    let mint = Keypair::new();
    if any_missing {
        create_mint(provider, &mint, &provider.payer.address(), 6).expect("create_mint for buyback vaults");
    }

    for i in 0..8u8 {
        if !missing[i as usize] {
            continue;
        }
        let buyback_vault_pda = vaults[i as usize];
        let (_, buyback_vault_bump) = Address::find_program_address(&[&BUYBACK_VAULT_SEED, &[i]], &PROGRAM_ID);
        let (buyback_vault_ata, _) = Address::find_program_address(
            &[buyback_vault_pda.as_ref(), TOKEN_PROGRAM_ID.as_ref(), mint.address().as_ref()],
            &ASSOCIATED_TOKEN_PROGRAM_ID,
        );

        build_initialize_buyback(
            provider,
            PROGRAM_ID,
            i,
            buyback_vault_bump,
            InitializeBuybackAccounts {
                payer: provider.payer.address(),
                buyback_vault: buyback_vault_pda,
                buyback_vault_ata,
                system_program: SYSTEM_PROGRAM_ID,
                associated_token_program: ASSOCIATED_TOKEN_PROGRAM_ID,
                mint: mint.address(),
                token_program: TOKEN_PROGRAM_ID,
            },
        )
        .log()
        .send_and_confirm()
        .expect("initialize_buyback should succeed");
    }

    vaults
}

#[test]
fn full_lifecycle() {
    let provider = setup();

    let pump_global_pda = ensure_pump_global(&provider);
    let fee_program_global_pda = ensure_fee_program_global(&provider, pump_global_pda);
    let fee_config_pda = ensure_fee_config(&provider);
    let buyback_vaults = ensure_buyback_vaults(&provider);

    let fee_program_global = fetch_fee_program_global(&provider, &fee_program_global_pda)
        .expect("fee_program_global should be readable");
    assert_eq!(fee_program_global.disable_flags, 0);

    let fee_config = fetch_fee_config(&provider, &fee_config_pda).expect("fee_config should be readable");
    assert_eq!(fee_config.fee_tiers_len, 1, "ensure_fee_config should have upserted the real zero-threshold tier");
    assert_eq!(fee_config.fee_tiers[0].fees.protocol_fee_bps, 95);
    assert_eq!(fee_config.fee_tiers[0].fees.creator_fee_bps, 30);
    // `is_new_quote_mint` (any non-SOL-quote `buy_v2`/`sell_v2` trade) selects
    // `stable_fee_tiers` instead of `fee_tiers` -- confirmed via
    // `reference/fee-tier-probe/src/bin/probe5.rs` against real deployed
    // `pump_fees.so`. Asserted explicitly here (not just "ensure_fee_config
    // ran without erroring") so a real regression -- this table silently
    // ending up empty again -- fails loudly here instead of surfacing later
    // as an opaque `NoFeeTiers` deep inside an unrelated `buy_v2` call.
    assert_eq!(fee_config.stable_fee_tiers_len, 1, "ensure_fee_config should have upserted the real zero-threshold stable tier");
    assert_eq!(fee_config.stable_fee_tiers[0].fees.protocol_fee_bps, 95);
    assert_eq!(fee_config.stable_fee_tiers[0].fees.creator_fee_bps, 30);

    for vault in buyback_vaults {
        assert!(provider.get_account_data(&vault).is_ok(), "buyback vault {vault} should exist");
    }
}
