use naclac_lang::prelude::*;
use crate::components::Global;
use crate::constants::GLOBAL_SEED;
use crate::errors::PumpError;

#[derive(Accounts)]
#[instruction(enabled: Bool)]
pub struct ToggleCashbackEnabled {
    #[account(mut, seeds = [GLOBAL_SEED], bump)]
    pub global: Account<Global>,

    #[account(mut)]
    pub authority: Signer,
}

pub fn toggle_cashback_enabled(ctx: Context<ToggleCashbackEnabled>, enabled: Bool) -> Result {
    require!(
        ctx.accounts.authority.address() == ctx.accounts.global.authority,
        PumpError::NotAuthorized
    );

    ctx.accounts.global.is_cashback_enabled = enabled;

    msg!("Cashback toggle successfully updated");
    Ok(())
}
