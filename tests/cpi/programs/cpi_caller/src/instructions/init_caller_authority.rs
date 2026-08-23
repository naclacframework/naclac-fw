use naclac_lang::prelude::*;
use crate::components::CallerAuthority;
use crate::constants::SEED_CALLER_AUTHORITY;

#[derive(Accounts)]
pub struct InitCallerAuthority {
    #[account(mut)]
    pub payer: Signer,

    #[account(
        init,
        payer = payer,
        seeds = [SEED_CALLER_AUTHORITY],
        bump
    )]
    pub caller_authority: Account<CallerAuthority>,

    pub system_program: Program<System>,
}

#[instruction]
pub fn init_caller_authority(ctx: Context<InitCallerAuthority>) -> Result {
    // Bare `bump`'s auto-write-back is zero-copy-only (see the same
    // gotcha, fixed the same way, in tests/token/'s init_mint_authority
    // and tests/events/'s init_counter) — written explicitly here so this
    // case works correctly in solana-borsh mode too, if it's ever extended
    // to that mode.
    ctx.accounts.caller_authority.bump = ctx.bumps.caller_authority;
    Ok(())
}
