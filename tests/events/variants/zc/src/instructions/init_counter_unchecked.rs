use naclac_lang::prelude::*;
use crate::components::Counter;
use crate::constants::SEED_COUNTER;

#[derive(Accounts)]
pub struct InitCounterUnchecked {
    #[account(mut)]
    pub payer: Signer,

    #[account(
        init_unchecked,
        payer = payer,
        seeds = [SEED_COUNTER],
        bump
    )]
    pub counter: Account<Counter>,

    pub system_program: Program<System>,
}

pub fn init_counter_unchecked(ctx: Context<InitCounterUnchecked>) -> Result {
    ctx.accounts.counter.bump = ctx.bumps.counter;
    ctx.accounts.counter.count = 41;
    Ok(())
}
