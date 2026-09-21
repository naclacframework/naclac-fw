use naclac_lang::prelude::*;
use crate::components::Global;
use crate::constants::GLOBAL_SEED;
use crate::errors::PumpError;

#[derive(Accounts)]
#[instruction(enabled: Bool)]
pub struct ToggleCreateV2 {
    #[account(mut, seeds = [GLOBAL_SEED], bump)]
    pub global: Account<Global>,

    #[account(mut)]
    pub authority: Signer,
}

pub fn toggle_create_v2(ctx: Context<ToggleCreateV2>, enabled: Bool) -> Result {
    require!(
        ctx.accounts.authority.address() == ctx.accounts.global.authority,
        PumpError::NotAuthorized
    );

    ctx.accounts.global.create_v2_enabled = enabled;

    msg!("create_v2 toggle successfully updated");
    Ok(())
}
