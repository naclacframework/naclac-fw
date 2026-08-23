use naclac_lang::prelude::*;
use crate::components::{FeeConfig, FeeTier};
use crate::constants::{FEE_CONFIG_SEED, MAX_FEE_TIERS};
use crate::errors::FeesError;
use crate::events::UpdateStableFeeConfigEvent;
use crate::systems::validate_fee_tiers;

// `config_program_id` must be declared before `fee_config`: its seeds
// reference `config_program_id.address()`, and sibling-field seed
// references must come after the field they reference.
#[derive(Accounts)]
pub struct UpdateStableFeeConfig {
    pub admin: Signer,
    /// SAFETY: only used as PDA seed material for `fee_config`; not
    /// deserialized or invoked.
    pub config_program_id: AccountInfo,
    #[account(mut, seeds = [FEE_CONFIG_SEED, config_program_id.address().as_ref()], bump)]
    pub fee_config: Account<FeeConfig>,
}

/// Set/Replace fee parameters entirely (only callable by admin)
#[instruction]
pub fn update_stable_fee_config(
    ctx: Context<UpdateStableFeeConfig>,
    stable_fee_tiers: ZcVec<FeeTier>,
) -> Result {
    require!(
        ctx.accounts.admin.address() == ctx.accounts.fee_config.admin,
        FeesError::InvalidAdmin
    );
    require!(stable_fee_tiers.len() <= MAX_FEE_TIERS, FeesError::TooManyFeeTiers);

    let timestamp = unix_timestamp()?;
    let cfg = &mut ctx.accounts.fee_config;
    for (i, tier) in stable_fee_tiers.iter().enumerate() {
        cfg.stable_fee_tiers[i] = tier;
    }
    cfg.stable_fee_tiers_len = stable_fee_tiers.len() as u32;
    let flat_fees = cfg.flat_fees;
    validate_fee_tiers(&cfg.stable_fee_tiers[..cfg.stable_fee_tiers_len as usize])?;

    // `UpdateStableFeeConfigEvent` needs an owned `Vec<FeeTier>`: `#[event(alloc)]`
    // only recognizes Vec/String/Option field kinds, not `ZcVec`/`Span`.
    let stable_fee_tiers: Vec<FeeTier> = stable_fee_tiers.iter().collect();
    emit!(UpdateStableFeeConfigEvent {
        timestamp,
        admin: ctx.accounts.admin.address(),
        fee_config: ctx.accounts.fee_config.address(),
        stable_fee_tiers,
        flat_fees,
    });

    Ok(())
}
