use naclac_lang::prelude::*;
use crate::components::{FeeConfig, Fees};
use crate::constants::{FEE_CONFIG_SEED, PUMP_AUTHORITY_SEED, PUMP_PROGRAM_ID};
use crate::systems::calculate_fee_tier;

// `config_program_id` must be declared before `fee_config`: its seeds
// reference `config_program_id.address()`, and sibling-field seed
// references must come after the field they reference.
#[derive(Accounts)]
pub struct GetFees {
    /// SAFETY: only used as PDA seed material for `fee_config` below; never
    /// deserialized, invoked, or otherwise trusted for its own contents.
    pub config_program_id: AccountInfo,
    #[account(seeds = [FEE_CONFIG_SEED, config_program_id.address().as_ref()], bump)]
    pub fee_config: Account<FeeConfig>,

    /// SAFETY: `signer` + the `seeds`/`seeds::program` constraint together
    /// prove this call was CPI'd (via `invoke_signed`) by `pump` itself —
    /// only that program can ever produce a valid signature for its own
    /// `PUMP_AUTHORITY_SEED` PDA. This is the entire authorization model
    /// for this instruction; never deserialized.
    #[account(
        signer,
        seeds = [PUMP_AUTHORITY_SEED],
        seeds::program = PUMP_PROGRAM_ID,
        bump,
    )]
    pub pump_authority: AccountInfo,
}

// fee-04 §5 / fees-06#2 (resolved via reference/fee-tier-probe/src/bin/probe5.rs):
// is_pump_pool selects flat_fees vs a tiered table; is_new_quote_mint then
// picks stable_fee_tiers vs fee_tiers, but only when is_pump_pool is true.
// trade_size_lamports is accepted (matching the real instruction's args) but,
// per fees-04 §3, no source material shows it affecting the result — unused.
//
// is_pump_pool/is_new_quote_mint are `Bool` (a Pod-safe u8 wrapper), not
// `bool` directly — `bool` has no guaranteed-valid arbitrary bit pattern, so
// it can't be zero-copy-parsed as an instruction arg.
/// Get Fees
#[instruction]
pub fn get_fees(
    ctx: Context<GetFees>,
    is_pump_pool: Bool,
    market_cap_lamports: u128,
    _trade_size_lamports: u64,
    is_new_quote_mint: Bool,
) -> Result<Fees> {
    let cfg = &ctx.accounts.fee_config;
    let is_pump_pool: bool = is_pump_pool.into();
    let is_new_quote_mint: bool = is_new_quote_mint.into();

    let fees = if is_pump_pool {
        let table = if is_new_quote_mint {
            &cfg.stable_fee_tiers[..cfg.stable_fee_tiers_len as usize]
        } else {
            &cfg.fee_tiers[..cfg.fee_tiers_len as usize]
        };
        calculate_fee_tier(table, market_cap_lamports)?
    } else {
        cfg.flat_fees
    };

    Ok(fees)
}
