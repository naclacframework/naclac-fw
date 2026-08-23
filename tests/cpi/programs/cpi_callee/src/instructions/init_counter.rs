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

    /// SAFETY: only this account's own address is read (to record as
    /// `counter.authority`) — its data is never read or deserialized, and
    /// it isn't required to sign here (only `authorized_increment` later
    /// requires it to sign).
    pub authority: AccountInfo,

    pub system_program: Program<System>,
}

#[instruction]
pub fn init_counter(ctx: Context<InitCounter>) -> Result {
    ctx.accounts.counter.value = 0;
    ctx.accounts.counter.authority = ctx.accounts.authority.address();
    Ok(())
}
