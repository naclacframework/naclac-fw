use naclac_lang::prelude::*;
use crate::components::FeeConfig;
use crate::constants::FEE_CONFIG_SEED;
use crate::errors::FeesError;
use crate::events::UpdateAdminEvent;

// `config_program_id` must be declared before `fee_config`: its seeds
// reference `config_program_id.address()`, and sibling-field seed
// references must come after the field they reference.
#[derive(Accounts)]
pub struct UpdateAdmin {
    pub admin: Signer,
    /// SAFETY: only used as PDA seed material for `fee_config`; not
    /// deserialized or invoked.
    pub config_program_id: AccountInfo,
    #[account(mut, seeds = [FEE_CONFIG_SEED, config_program_id.address().as_ref()], bump)]
    pub fee_config: Account<FeeConfig>,
    /// SAFETY: new admin is passed as an account (not instruction data);
    /// its pubkey is only stored, never deserialized or invoked.
    pub new_admin: AccountInfo,
}

/// Update admin (only callable by admin)
pub fn update_admin(ctx: Context<UpdateAdmin>) -> Result {
    require!(
        ctx.accounts.admin.address() == ctx.accounts.fee_config.admin,
        FeesError::InvalidAdmin
    );

    let timestamp = unix_timestamp()?;
    let old_admin = ctx.accounts.fee_config.admin;
    let new_admin = ctx.accounts.new_admin.address();

    ctx.accounts.fee_config.admin = new_admin;

    emit!(UpdateAdminEvent {
        timestamp,
        old_admin,
        new_admin,
    });

    Ok(())
}
