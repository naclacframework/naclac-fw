use naclac_lang::prelude::*;

pub const AMM_CREATOR_VAULT_AUTHORITY_SEED: &[u8] = b"creator_vault";
pub const WSOL_MINT: Address = address!("So11111111111111111111111111111111111111112");

// pump's declared program ID (examples/pump-fun-program/pump-bonding-curve).
pub const PUMP_PROGRAM_ID: Address = address!("FoN4cWC8wuVYK3Dd2ge1WVTLpPUvj4CcWXZsq4wmadwD");
pub const PUMP_CREATOR_VAULT_SEED: &[u8] = b"creator-vault";

pub const POOL_SEED: &[u8] = b"pool";
pub const POOL_LP_MINT_SEED: &[u8] = b"pool_lp_mint";
pub const GLOBAL_CONFIG_SEED: &[u8] = b"global_config";

/// Real `pump_amm`'s `create_config` admin and real `pump_fees`'s
/// `initialize_fee_config` admin are the same pubkey on mainnet
/// (`8LWu7QM2dGR1G8nKDHthckea57bkCzXyBTAKPJUBDHo8`, confirmed via both
/// programs' real IDLs) — but this project doesn't hold that key's private
/// half, so (matching `pump_fees::ADMIN_PUBKEY`'s own precedent) this is a
/// project-generated pubkey instead, reusing the exact same one
/// `pump_fees::ADMIN_PUBKEY` already uses and the same `tests/wallets/test_admin.json`
/// keypair, mirroring the real one-admin-for-both structure.
pub const ADMIN_PUBKEY: Address = address!("ASxCm9nmuYJkJLx56GJjHHHw96DwFgnfqq2CTkWSEz9q");

/// `GlobalConfig.disable_flags` bit 0 — `create_pool` disabled when set.
pub const DISABLE_CREATE_POOL_FLAG: u8 = 1 << 0;

/// `lp_minted = floor(sqrt(base_amount_in * quote_amount_in)) - 100` — the
/// 100 units permanently withheld from the depositor's own LP balance (no
/// separate burn/lock account; confirmed via
/// `reference/fee-tier-probe/src/bin/probe14.rs` against real bytecode).
pub const LP_BOOTSTRAP_WITHHELD: u128 = 100;

/// Confirmed against a freshly re-dumped `pump_amm.so`'s real `lp_mint`
/// account bytes (`probe14`, byte offset 44 = `09`), not assumed.
pub const LP_MINT_DECIMALS: u8 = 9;

pub const BOOST_VAULT_SEED: &[u8] = b"boost_vault";

/// `init_boost`'s real `virtual_quote_reserves = floor(quote_amount_in *
/// base_amount_in / BOOST_BASE_SUPPLY_DIVISOR)` — confirmed via 5
/// independent real mainnet `migrate_v2` transactions (zero error across
/// all 5) plus a direct blind litesvm replication using fresh,
/// previously-untested numbers and a 9-decimal quote mint (ruling out a
/// quote-decimals dependency). `base_amount_in` here is `pool_base_token_account`'s
/// balance at the moment `init_boost` executes, not any value read from
/// `Pool`'s own stored fields. This divisor equals pump.fun's standard
/// 1,000,000,000-token / 6-decimal total base supply (1e15) — a fixed
/// constant, not derived from `base_mint.supply` (confirmed: real supply
/// was 0 in every probe fixture yet the formula held exactly).
pub const BOOST_BASE_SUPPLY_DIVISOR: u128 = 1_000_000_000_000_000;
