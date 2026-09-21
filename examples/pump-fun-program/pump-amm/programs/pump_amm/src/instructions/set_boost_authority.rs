use naclac_lang::prelude::*;
use crate::components::GlobalConfig;
use crate::constants::GLOBAL_CONFIG_SEED;
use crate::errors::PumpAmmError;

#[derive(Accounts)]
pub struct SetBoostAuthority {
    pub admin: Signer,

    #[account(mut, seeds = [GLOBAL_CONFIG_SEED], bump)]
    pub global_config: Account<GlobalConfig>,

    /// SAFETY: the new `boost_authority` value is this account's own
    /// address (real `set_boost_authority` takes zero args, matching this)
    /// — never deserialized, only its address is read.
    pub boost_authority: AccountInfo,

    pub system_program: Program<System>,
}

pub fn set_boost_authority(ctx: Context<SetBoostAuthority>) -> Result {
    require!(
        ctx.accounts.admin.address() == ctx.accounts.global_config.admin,
        PumpAmmError::InvalidAdmin
    );
    ctx.accounts.global_config.boost_authority = ctx.accounts.boost_authority.address();
    Ok(())
}
