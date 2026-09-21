use naclac_lang::prelude::*;
use crate::components::FeeProgramGlobal;
use crate::constants::FEE_PROGRAM_GLOBAL_SEED;
use crate::errors::FeesError;
use crate::events::SetAuthorityEvent;

#[derive(Accounts)]
pub struct SetAuthority {
    pub authority: Signer,
    #[account(mut, seeds = [FEE_PROGRAM_GLOBAL_SEED])]
    pub fee_program_global: Account<FeeProgramGlobal>,
}

pub fn set_authority(ctx: Context<SetAuthority>, new_authority: Address) -> Result {
    require!(
        ctx.accounts.authority.address() == ctx.accounts.fee_program_global.authority,
        FeesError::InvalidAdmin
    );

    let timestamp = unix_timestamp()?;
    let old_authority = ctx.accounts.fee_program_global.authority;
    ctx.accounts.fee_program_global.authority = new_authority;

    emit!(SetAuthorityEvent {
        timestamp,
        old_authority,
        new_authority,
    });

    Ok(())
}
