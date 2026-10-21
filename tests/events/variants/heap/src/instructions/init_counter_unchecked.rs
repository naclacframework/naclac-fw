use naclac_lang::prelude::*;
use crate::components::Counter;
use crate::constants::SEED_COUNTER;

/// Creates the counter through `init_unchecked`, which calls naclac-core's
/// raw `sol_invoke_signed_rust`-based `CreateAccount` on the Solana backend.
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
