use naclac_lang::prelude::*;
use crate::components::BigData;
use crate::constants::SEED_BIG;

#[derive(Accounts)]
pub struct InitBig {
    #[account(mut)]
    pub payer: Signer,

    #[account(
        init,
        payer = payer,
        space = 8 + core::mem::size_of::<BigData>(),
        seeds = [SEED_BIG],
        bump
    )]
    pub big: Box<Account<BigData>>,

    pub system_program: Program<System>,
}

pub fn init_big(ctx: Context<InitBig>) -> Result {
    let big = &mut ctx.accounts.big;
    big.bump = ctx.bumps.big;
    big.payload = [0u8; 300];
    Ok(())
}
