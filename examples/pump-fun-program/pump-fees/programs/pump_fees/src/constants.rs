use naclac_lang::prelude::*;

pub const FEE_CONFIG_SEED: &[u8] = b"fee_config";
pub const FEE_PROGRAM_GLOBAL_SEED: [u8; 18] = *b"fee-program-global";
#[constant]
pub const SHARING_CONFIG_SEED: [u8; 14] = *b"sharing-config";
pub const SOCIAL_FEE_PDA_SEED: [u8; 14] = *b"social-fee-pda";
#[constant]
pub const DONATION_FEE_PDA_SEED: [u8; 16] = *b"donation-fee-pda";
pub const BUYBACK_VAULT_SEED: [u8; 13] = *b"buyback-vault";
/// Bonding Curve Program Global Seed
pub const PUMP_GLOBAL_SEED: [u8; 6] = *b"global";
/// Signer-only PDA (no stored data) that `pump_fees` CPI-signs with to prove
/// to `pump::migrate_bonding_curve_creator` that the call genuinely came
/// from this program, not an untrusted direct caller. Must match `pump`'s
/// own `PUMP_FEES_AUTHORITY_SEED` exactly.
pub const PUMP_FEES_AUTHORITY_SEED: &[u8] = b"pump-fees-authority";
/// `pump`'s own signer-only authority PDA — proves a CPI into `get_fees`
/// genuinely came from `pump`, not an untrusted direct caller. Must match
/// `pump`'s own `PUMP_AUTHORITY_SEED` exactly.
pub const PUMP_AUTHORITY_SEED: &[u8] = b"pump-authority";
pub const BONDING_CURVE_SEED: &[u8] = b"bonding-curve";
/// pump-amm's per-mint pool-authority seed (migration-time canonical pool owner).
pub const POOL_AUTHORITY_SEED: &[u8] = b"pool-authority";
/// pump-amm's pool seed: `[POOL_SEED, index, pool_authority, base_mint, quote_mint]`.
pub const POOL_SEED: &[u8] = b"pool";

/// Pump AMM's creator-vault-authority seed.
#[constant]
pub const AMM_CREATOR_VAULT_AUTHORITY_SEED: [u8; 13] = *b"creator_vault";
/// Pump bonding curve's creator-vault seed.
#[constant]
pub const PUMP_CREATOR_VAULT_SEED: [u8; 13] = *b"creator-vault";
/// `donation_relay_program`'s epoch-tracker satellite-account seed.
#[constant]
pub const EPOCH_TRACKER_V1: &[u8] = b"epoch_tracker_v1";
/// `donation_relay_program`'s debouncer satellite-account seed.
#[constant]
pub const DEBOUNCER_V1: &[u8] = b"debouncer_v1";
/// `donation_relay_program`'s mint-whitelist satellite-account seed.
#[constant]
pub const MINT_WHITELIST_V1: &[u8] = b"mint_whitelist_v1";
/// `donation_relay_program`'s `donate_pubkey_config_id_with_payer` instruction discriminator.
#[constant]
pub const IX_DONATE_PUBKEY_CONFIG_ID_WITH_PAYER_V1: [u8; 8] =
    [120, 217, 57, 241, 135, 104, 139, 184];

pub const MAX_FEE_TIERS: usize = 50;
// Real `pump_fees::SharingConfig` accounts are a fixed 1024 bytes
// (confirmed against real deployed bytecode, `reference/fee-tier-probe/src/bin/probe70.rs`),
// which fits at most 27 shareholders (76-byte fixed prefix + 4-byte Borsh
// Vec length + 34 bytes each). 30 is our own chosen cap for this
// reimplementation — comfortably above the real ceiling, not required to
// match it exactly since this is our own independently-sized account.
pub const MAX_SHAREHOLDERS: usize = 30;
#[constant]
pub const MAX_BUYBACK_INDEX: u8 = 8;
pub const FEE_CONFIG_CURRENT_SIZE: usize = 4073;

// Our reimplemented `pump` bonding-curve program's declared ID (examples/pump-fun-program/pump-bonding-curve).
pub const PUMP_PROGRAM_ID: Address = address!("FoN4cWC8wuVYK3Dd2ge1WVTLpPUvj4CcWXZsq4wmadwD");
// Our reimplemented `pump_amm` program's declared ID (examples/pump-fun-program/pump-amm).
pub const PUMP_AMM_PROGRAM_ID: Address = address!("HymVkySKqosA3Qhhwg8cwkjMCEk815HYyBRzEwa8huPx");
// Our reimplemented `donation_relay` program's declared ID (examples/donation-relay).
pub const DONATION_RELAY_PROGRAM_ID: Address = address!("2abJkQX74rXzAJEgKRq8PmrT62M2iFtachKGqc4wn9tX");

pub const WSOL_MINT: Address = address!("So11111111111111111111111111111111111111112");

pub const ADMIN_PUBKEY: Address = address!("ASxCm9nmuYJkJLx56GJjHHHw96DwFgnfqq2CTkWSEz9q");
