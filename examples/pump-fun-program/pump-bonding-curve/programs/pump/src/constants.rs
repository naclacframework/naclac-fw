 use naclac_lang::prelude::*;

pub const GLOBAL_SEED: &[u8] = b"global";
pub const BONDING_CURVE_SEED: &[u8] = b"bonding-curve";
pub const MINT_AUTHORITY_SEED: &[u8] = b"mint-authority";

pub const PUMP_AUTHORITY_SEED: &[u8] = b"pump-authority";

pub const MINT_DECIMALS: u8 = 6;

pub const PUMP_FEES_PROGRAM_ID: Address = address!("8rG6Zs43yJ71tkCsWoEqdxF1uN9HzpQnCS8huqkKuwPJ");

pub const PUMP_FEES_AUTHORITY_SEED: &[u8] = b"pump-fees-authority";
pub const SHARING_CONFIG_SEED: &[u8] = b"sharing-config";
pub const CREATOR_VAULT_SEED: &[u8] = b"creator-vault";
// Must match `pump_fees`'s own `MAX_SHAREHOLDERS` exactly -- this program
// reads `sharing_config` accounts `pump_fees` creates.
pub const MAX_SHAREHOLDERS: usize = 30;
pub const WSOL_MINT: Address = address!("So11111111111111111111111111111111111111112");

pub const GLOBAL_VOLUME_ACCUMULATOR_SEED: &[u8] = b"global_volume_accumulator";
pub const USER_VOLUME_ACCUMULATOR_SEED: &[u8] = b"user_volume_accumulator";

pub const FEE_CONFIG_SEED: &[u8] = b"fee_config";

pub const BPS_DENOMINATOR: u64 = 10_000;

pub const BUYBACK_VAULT_SEED: &[u8] = b"buyback-vault";

pub const MPL_TOKEN_METADATA_PROGRAM_ID: Address = address!("metaqbxxUerdq28cj1RbAWkYQm3ybzjb6a8bt518x1s");
pub const METADATA_SEED: &[u8] = b"metadata";

pub const POOL_AUTHORITY_SEED: &[u8] = b"pool-authority";
/// pump-amm's pool seed: `[POOL_SEED, index, pool_authority, base_mint, quote_mint]`.
pub const POOL_SEED: &[u8] = b"pool";
pub const POOL_LP_MINT_SEED: &[u8] = b"pool_lp_mint";
pub const GLOBAL_CONFIG_SEED: &[u8] = b"global_config";
pub const PUMP_AMM_PROGRAM_ID: Address = address!("HymVkySKqosA3Qhhwg8cwkjMCEk815HYyBRzEwa8huPx");
pub const ASSOCIATED_TOKEN_PROGRAM_ID: Address = address!("ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL");
pub const BOOST_VAULT_SEED: &[u8] = b"boost_vault";

/// Real, confirmed via `reference/pump-rust-client/src/pda.rs`'s own
/// `bonding_curve_v2` helper and cross-checked against a real mainnet `buy`
/// transaction's actual account list
/// (`reference/fee-tier-probe/src/bin/probe53.rs`).
pub const BONDING_CURVE_V2_SEED: &[u8] = b"bonding-curve-v2";

/// Real, confirmed via `reference/fee-tier-probe/src/bin/probe51.rs` (a real
/// `buy_v2` on a fresh `creator_vault` topped it up by exactly this amount
/// before crediting `creator_fee`; a second real trade on an
/// already-funded `creator_vault` added no top-up at all) — the standard
/// Solana rent-exempt minimum for a zero-data account.
#[constant]
pub const CREATOR_VAULT_RENT_EXEMPT_MINIMUM: u64 = 890_880;
