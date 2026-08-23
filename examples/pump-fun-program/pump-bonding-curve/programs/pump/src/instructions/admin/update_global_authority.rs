use naclac_lang::prelude::*;
use crate::components::Global;
use crate::constants::GLOBAL_SEED;
use crate::errors::PumpError;
use crate::events::UpdateGlobalAuthorityEvent;

#[derive(Accounts)]
pub struct UpdateGlobalAuthority {
    #[account(mut, seeds = [GLOBAL_SEED], bump)]
    pub global: Account<Global>,

    #[account(mut)]
    pub authority: Signer,

    /// SAFETY: only its address is read, written into `global.authority` --
    /// never deserialized.
    pub new_authority: AccountInfo,
}

#[instruction]
pub fn update_global_authority(ctx: Context<UpdateGlobalAuthority>) -> Result {
    require!(
        ctx.accounts.authority.address() == ctx.accounts.global.authority,
        PumpError::NotAuthorized
    );

    let new_authority = ctx.accounts.new_authority.address();
    ctx.accounts.global.authority = new_authority;

    let timestamp = unix_timestamp()?;
    emit!(UpdateGlobalAuthorityEvent {
        global: ctx.accounts.global.address(),
        authority: ctx.accounts.authority.address(),
        new_authority,
        timestamp,
    });

    msg!("Global authority successfully updated");
    Ok(())
}
