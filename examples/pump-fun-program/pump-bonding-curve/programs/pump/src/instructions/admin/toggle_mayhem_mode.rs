use naclac_lang::prelude::*;
use crate::components::Global;
use crate::constants::GLOBAL_SEED;
use crate::errors::PumpError;

#[derive(Accounts)]
#[instruction(enabled: Bool)]
pub struct ToggleMayhemMode {
    #[account(mut, seeds = [GLOBAL_SEED], bump)]
    pub global: Account<Global>,

    #[account(mut)]
    pub authority: Signer,
}

#[instruction]
pub fn toggle_mayhem_mode(ctx: Context<ToggleMayhemMode>, enabled: Bool) -> Result {
    require!(
        ctx.accounts.authority.address() == ctx.accounts.global.authority,
        PumpError::NotAuthorized
    );

    ctx.accounts.global.mayhem_mode_enabled = enabled;

    msg!("Mayhem mode toggle successfully updated");
    Ok(())
}
