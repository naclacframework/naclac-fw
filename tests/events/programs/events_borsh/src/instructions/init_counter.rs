use naclac_lang::prelude::*;
use crate::components::Counter;
use crate::constants::SEED_COUNTER;

#[derive(Accounts)]
pub struct InitCounter {
    #[account(mut)]
    pub payer: Signer,

    #[account(
        init,
        payer = payer,
        seeds = [SEED_COUNTER],
        bump
    )]
    pub counter: Account<Counter>,

    pub system_program: Program<System>,
}

pub fn init_counter(ctx: Context<InitCounter>) -> Result {
    // Bare `bump`'s auto-write-back is zero-copy-only (see
    // `init_mint_authority` in tests/token/ for the same gotcha) — without
    // this, `counter.bump` stays 0 forever in solana-borsh mode, which
    // `touch_counter_explicit_bump`'s explicit self-reference check
    // actually caught (bare bump's own verification is skipped entirely on
    // a precomputed-literal-seed existing account, so it never noticed).
    ctx.accounts.counter.bump = ctx.bumps.counter;
    ctx.accounts.counter.count = 0;
    Ok(())
}
