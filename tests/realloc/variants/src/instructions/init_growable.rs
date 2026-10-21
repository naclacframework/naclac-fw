use naclac_lang::prelude::*;
use crate::components::Growable;
use crate::constants::SEED_GROWABLE;

#[derive(Accounts)]
pub struct InitGrowable {
    #[account(mut)]
    pub payer: Signer,

    #[account(
        init,
        payer = payer,
        space = 8 + core::mem::size_of::<Growable>(),
        seeds = [SEED_GROWABLE],
        bump
    )]
    pub growable: Account<Growable>,

    pub system_program: Program<System>,
}

pub fn init_growable(ctx: Context<InitGrowable>) -> Result {
    ctx.accounts.growable.bump = ctx.bumps.growable;
    ctx.accounts.growable.tag = 0;
    Ok(())
}
