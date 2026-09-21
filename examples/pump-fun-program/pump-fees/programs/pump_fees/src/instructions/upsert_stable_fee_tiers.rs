use naclac_lang::prelude::*;
use crate::components::{FeeConfig, FeeTier};
use crate::constants::{FEE_CONFIG_SEED, MAX_FEE_TIERS};
use crate::errors::FeesError;
use crate::events::UpsertStableFeeTiersEvent;
use crate::systems::validate_fee_tiers;

// `config_program_id` must be declared before `fee_config`: its seeds
// reference `config_program_id.address()`, and sibling-field seed
// references must come after the field they reference.
#[derive(Accounts)]
pub struct UpsertStableFeeTiers {
    pub admin: Signer,
    /// SAFETY: only used as PDA seed material for `fee_config`; not
    /// deserialized or invoked.
    pub config_program_id: AccountInfo,
    #[account(mut, seeds = [FEE_CONFIG_SEED, config_program_id.address().as_ref()], bump)]
    pub fee_config: Account<FeeConfig>,
}

/// Update or expand fee tiers (only callable by admin)
pub fn upsert_stable_fee_tiers(
    ctx: Context<UpsertStableFeeTiers>,
    stable_fee_tiers: ZcVec<FeeTier>,
    offset: u8,
) -> Result {
    require!(
        ctx.accounts.admin.address() == ctx.accounts.fee_config.admin,
        FeesError::InvalidAdmin
    );

    let cfg = &mut ctx.accounts.fee_config;
    let offset = offset as usize;
    let current_len = cfg.stable_fee_tiers_len as usize;
    require!(offset <= current_len, FeesError::OffsetNotContinuous);

    let new_len = core::cmp::max(current_len, offset + stable_fee_tiers.len());
    require!(new_len <= MAX_FEE_TIERS, FeesError::TooManyFeeTiers);

    for (i, tier) in stable_fee_tiers.iter().enumerate() {
        cfg.stable_fee_tiers[offset + i] = tier;
    }
    cfg.stable_fee_tiers_len = new_len as u32;

    // Re-validates the whole table, not just the spliced range — a splice
    // can break sort order at either boundary (fees-05 cross-cutting #7).
    validate_fee_tiers(&cfg.stable_fee_tiers[..new_len])?;

    let timestamp = unix_timestamp()?;
    // `UpsertStableFeeTiersEvent` needs an owned `Vec<FeeTier>`: `#[event(alloc)]`
    // only recognizes Vec/String/Option field kinds, not `ZcVec`/`Span`.
    let stable_fee_tiers: Vec<FeeTier> = stable_fee_tiers.iter().collect();
    emit!(UpsertStableFeeTiersEvent {
        timestamp,
        admin: ctx.accounts.admin.address(),
        fee_config: ctx.accounts.fee_config.address(),
        stable_fee_tiers,
        offset: offset as u8,
    });

    Ok(())
}
