use naclac_lang::prelude::*;
use crate::components::FeeConfig;
use crate::constants::{FEE_CONFIG_CURRENT_SIZE, FEE_CONFIG_SEED};
use crate::events::ExtendFeeConfigEvent;

// `config_program_id` must be declared before `fee_config`: its seeds
// reference `config_program_id.address()`, and sibling-field seed
// references must come after the field they reference.
#[derive(Accounts)]
pub struct ExtendFeeConfig {
    #[account(mut)]
    pub user: Signer,
    /// SAFETY: only used as PDA seed material for `fee_config`; not
    /// deserialized or invoked.
    pub config_program_id: AccountInfo,
    #[account(
        mut,
        seeds = [FEE_CONFIG_SEED, config_program_id.address().as_ref()],
        bump,
        realloc = FEE_CONFIG_CURRENT_SIZE,
        realloc::payer = user,
        realloc::zero = false
    )]
    pub fee_config: Account<FeeConfig>,
    pub system_program: Program<System>,
}

/// Realloc the `fee_config` PDA to `FeeConfig::CURRENT_SIZE`. Callable by
/// any signer (`user` pays the rent delta, per the real program's IDL —
/// there's no admin/ownership check on this instruction).
#[instruction]
pub fn extend_fee_config(ctx: Context<ExtendFeeConfig>) -> Result {
    let current_size = ctx.accounts.fee_config.to_account_info().data().len() as u64;
    let timestamp = unix_timestamp()?;

    emit!(ExtendFeeConfigEvent {
        current_size,
        new_size: FEE_CONFIG_CURRENT_SIZE as u64,
        timestamp,
        fee_config: ctx.accounts.fee_config.address(),
        user: ctx.accounts.user.address(),
    });

    Ok(())
}
