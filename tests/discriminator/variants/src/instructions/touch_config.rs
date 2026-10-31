use naclac_lang::prelude::*;
use crate::state::Config;

// `config` is `mut` with no `seeds`/`address` constraint, so the only thing
// standing between a correctly-typed Config and an arbitrary same-owner
// account (e.g. a Vault) is `Account<Config>`'s discriminator check on the
// `mut` load path.
#[derive(Accounts)]
pub struct TouchConfig {
    pub caller: Signer,

    #[account(mut)]
    pub config: Account<Config>,
}

pub fn touch_config(ctx: Context<TouchConfig>) -> Result {
    let _fee_bps = ctx.accounts.config.fee_bps;
    Ok(())
}
