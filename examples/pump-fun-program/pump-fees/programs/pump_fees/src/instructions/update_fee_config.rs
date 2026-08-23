use naclac_lang::prelude::*;
use crate::components::{Fees, FeeConfig, FeeTier};
use crate::constants::{FEE_CONFIG_SEED, MAX_FEE_TIERS};
use crate::errors::FeesError;
use crate::events::UpdateFeeConfigEvent;
use crate::systems::validate_fee_tiers;

// `config_program_id` must be declared before `fee_config`: its seeds
// reference `config_program_id.address()`, and sibling-field seed
// references must come after the field they reference.
#[derive(Accounts)]
pub struct UpdateFeeConfig {
    pub admin: Signer,
    /// SAFETY: only used as PDA seed material for `fee_config`; not
    /// deserialized or invoked.
    pub config_program_id: AccountInfo,
    #[account(mut, seeds = [FEE_CONFIG_SEED, config_program_id.address().as_ref()], bump)]
    pub fee_config: Account<FeeConfig>,
}

/// Set/Replace fee parameters entirely (only callable by admin)
#[instruction]
pub fn update_fee_config(
    ctx: Context<UpdateFeeConfig>,
    fee_tiers: ZcVec<FeeTier>,
    flat_fees: Fees,
) -> Result {
    require!(
        ctx.accounts.admin.address() == ctx.accounts.fee_config.admin,
        FeesError::InvalidAdmin
    );
    require!(fee_tiers.len() <= MAX_FEE_TIERS, FeesError::TooManyFeeTiers);

    let timestamp = unix_timestamp()?;
    let cfg = &mut ctx.accounts.fee_config;
    for (i, tier) in fee_tiers.iter().enumerate() {
        cfg.fee_tiers[i] = tier;
    }
    cfg.fee_tiers_len = fee_tiers.len() as u32;
    cfg.flat_fees = flat_fees;

    validate_fee_tiers(&cfg.fee_tiers[..cfg.fee_tiers_len as usize])?;

    // `UpdateFeeConfigEvent` needs an owned `Vec<FeeTier>`: `#[event(alloc)]`
    // only recognizes Vec/String/Option field kinds, not `ZcVec`/`Span`.
    let fee_tiers: Vec<FeeTier> = fee_tiers.iter().collect();
    emit!(UpdateFeeConfigEvent {
        timestamp,
        admin: ctx.accounts.admin.address(),
        fee_config: ctx.accounts.fee_config.address(),
        fee_tiers,
        flat_fees,
    });

    Ok(())
}
