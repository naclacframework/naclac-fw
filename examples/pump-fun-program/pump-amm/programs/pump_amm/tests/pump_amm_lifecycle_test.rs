//! Sequential, idempotent, cross-cluster lifecycle test for `pump_amm` --
//! the `pump_amm` counterpart to `pump-bonding-curve`'s own
//! `pump_lifecycle_test.rs` and `pump-fees`'s own `pump_fees_lifecycle_test.rs`.
//!
//! Real `pump_amm` has no instruction that creates `GlobalConfig` at all --
//! `pump_amm_test.rs`'s own litesvm tests only work because they hand-inject
//! its bytes directly via `set_account`, which has no equivalent on a real
//! cluster. `create_config` (this project's own addition -- see its own doc
//! comment) is what makes real on-chain `GlobalConfig` creation possible at
//! all, and this file is what actually exercises it for real, since nothing
//! else in `pump::migrate_v2`'s CPI chain can succeed on a real cluster
//! without it existing first.
//!
//! `create_config`'s golden path requires a signer matching `ADMIN_PUBKEY`
//! -- same project-generated keypair `pump_fees_lifecycle_test.rs` already
//! uses (`tests/wallets/test_admin.json`, copied here verbatim), since both
//! `ADMIN_PUBKEY` constants are the same pubkey (see that constant's own
//! doc comment for why).
//!
//! Cluster selection: edit the `CLUSTER` constant below, same as the other
//! two lifecycle test files.

use naclac_client::*;
use pump_amm_client::{
    fetch_global_config, get_global_config_pda,
    instructions::{
        build_create_config, build_set_boost_authority, build_toggle_boost, CreateConfigAccounts,
        SetBoostAuthorityAccounts, ToggleBoostAccounts,
    },
    PROGRAM_ID,
};
use std::path::PathBuf;

/// Change this to switch clusters -- "litesvm", "localnet", "devnet",
/// "mainnet", or a raw RPC URL (see `NaclacProvider::new`).
const CLUSTER: &str = "devnet";

fn setup() -> NaclacProvider {
    let payer = load_node_wallet().expect("Failed to load local Solana keypair");
    let provider = NaclacProvider::new(CLUSTER, payer);

    if provider.cluster == RpcCluster::Litesvm {
        let mut workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        workspace_root.pop(); // programs
        workspace_root.pop(); // pump-amm workspace root
        let so_path = resolve_cargo_target_dir(&workspace_root).join("deploy/pump_amm.so");
        provider
            .add_program(&PROGRAM_ID, so_path.to_str().unwrap())
            .expect("Failed to load pump_amm.so");
    }

    provider
}

/// `create_config`'s golden path requires a signer matching `ADMIN_PUBKEY`
/// -- same project-generated keypair `pump_fees_lifecycle_test.rs` uses
/// (see module comment for why the two `ADMIN_PUBKEY` constants match).
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

/// Idempotent: real `create_config` can only succeed once (`global_config`
/// is `init`-only). On first creation, also enables boost (`toggle_boost`)
/// and sets a real boost authority (`set_boost_authority`, to
/// `provider.payer` -- a project choice, not a value matched against real
/// mainnet, since `boost_authority`'s real intended holder isn't confirmed
/// anywhere in this project's research) -- `pump::migrate_v2` unconditionally
/// CPIs into `init_boost`, which requires `boost_enabled = true`.
fn ensure_global_config(provider: &NaclacProvider) -> Address {
    let (global_config_pda, _bump) = get_global_config_pda(&PROGRAM_ID);
    if fetch_global_config(provider, &global_config_pda).is_ok() {
        return global_config_pda;
    }

    let admin = load_test_admin_keypair();
    if provider.cluster == RpcCluster::Litesvm {
        provider.airdrop(&admin.address(), 10_000_000_000).unwrap();
    } else {
        assert!(
            provider.get_balance(&admin.address()).unwrap_or(0) > 0,
            "test admin wallet {} has no balance on this cluster -- fund it before running \
             pump_amm_lifecycle_test.rs here",
            admin.address()
        );
    }

    build_create_config(
        provider,
        PROGRAM_ID,
        CreateConfigAccounts {
            admin: admin.address(),
            global_config: global_config_pda,
            system_program: SYSTEM_PROGRAM_ID,
        },
    )
    .signer(&admin)
    .log()
    .send_and_confirm()
    .expect("create_config should succeed for the real ADMIN_PUBKEY signer");

    build_toggle_boost(
        provider,
        PROGRAM_ID,
        Bool::from(true),
        ToggleBoostAccounts { admin: admin.address(), global_config: global_config_pda },
    )
    .signer(&admin)
    .log()
    .send_and_confirm()
    .expect("toggle_boost should succeed");

    build_set_boost_authority(
        provider,
        PROGRAM_ID,
        SetBoostAuthorityAccounts {
            admin: admin.address(),
            global_config: global_config_pda,
            boost_authority: provider.payer.address(),
            system_program: SYSTEM_PROGRAM_ID,
        },
    )
    .signer(&admin)
    .log()
    .send_and_confirm()
    .expect("set_boost_authority should succeed");

    global_config_pda
}

#[test]
fn full_lifecycle() {
    let provider = setup();

    let global_config_pda = ensure_global_config(&provider);

    let global_config = fetch_global_config(&provider, &global_config_pda).expect("global_config should be readable");
    assert_eq!(global_config.disable_flags, 0);
    assert!(bool::from(global_config.boost_enabled), "boost should be enabled -- pump::migrate_v2 needs this");
    assert_eq!(global_config.boost_authority, provider.payer.address());
}
