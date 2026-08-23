use naclac_lang::prelude::*;
use crate::components::FeeProgramGlobal;
use crate::constants::FEE_PROGRAM_GLOBAL_SEED;
use crate::errors::FeesError;
use crate::events::SetDisableFlagsEvent;

#[derive(Accounts)]
pub struct SetDisableFlags {
    pub authority: Signer,
    #[account(mut, seeds = [FEE_PROGRAM_GLOBAL_SEED])]
    pub fee_program_global: Account<FeeProgramGlobal>,
}

#[instruction]
pub fn set_disable_flags(ctx: Context<SetDisableFlags>, disable_flags: u8) -> Result {
    require!(
        ctx.accounts.authority.address() == ctx.accounts.fee_program_global.authority,
        FeesError::InvalidAdmin
    );

    let timestamp = unix_timestamp()?;
    ctx.accounts.fee_program_global.disable_flags = disable_flags;

    emit!(SetDisableFlagsEvent {
        timestamp,
        disable_flags,
    });

    Ok(())
}
