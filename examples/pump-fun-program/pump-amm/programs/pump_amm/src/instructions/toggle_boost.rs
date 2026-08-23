use naclac_lang::prelude::*;
use crate::components::GlobalConfig;
use crate::constants::GLOBAL_CONFIG_SEED;
use crate::errors::PumpAmmError;

#[derive(Accounts)]
pub struct ToggleBoost {
    pub admin: Signer,

    #[account(mut, seeds = [GLOBAL_CONFIG_SEED], bump)]
    pub global_config: Account<GlobalConfig>,
}

#[instruction]
pub fn toggle_boost(ctx: Context<ToggleBoost>, enabled: Bool) -> Result {
    require!(
        ctx.accounts.admin.address() == ctx.accounts.global_config.admin,
        PumpAmmError::InvalidAdmin
    );
    ctx.accounts.global_config.boost_enabled = enabled;
    Ok(())
}
