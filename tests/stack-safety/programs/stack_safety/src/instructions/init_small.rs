use naclac_lang::prelude::*;
use crate::components::SmallData;
use crate::constants::SEED_SMALL;

#[derive(Accounts)]
pub struct InitSmall {
    #[account(mut)]
    pub payer: Signer,

    #[account(
        init,
        payer = payer,
        space = 8 + core::mem::size_of::<SmallData>(),
        seeds = [SEED_SMALL],
        bump
    )]
    pub small: Account<SmallData>,

    pub system_program: Program<System>,
}

#[instruction]
pub fn init_small(ctx: Context<InitSmall>) -> Result {
    let small = &mut ctx.accounts.small;
    small.bump = ctx.bumps.small;
    small.value = 0;
    Ok(())
}
