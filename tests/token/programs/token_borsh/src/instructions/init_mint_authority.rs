use naclac_lang::prelude::*;
use crate::components::MintAuthority;
use crate::constants::SEED_MINT_AUTHORITY;

#[derive(Accounts)]
pub struct InitMintAuthority {
    #[account(mut)]
    pub payer: Signer,

    #[account(
        init,
        payer = payer,
        seeds = [SEED_MINT_AUTHORITY],
        bump
    )]
    pub mint_authority: Account<MintAuthority>,

    pub system_program: Program<System>,
}

pub fn init_mint_authority(ctx: Context<InitMintAuthority>) -> Result {
    // Bare `bump` only auto-writes the derived bump back into the account
    // for zero-copy fields (`naclac-macros/src/accounts.rs`'s
    // `mut_zero_copy_fields`) — genuine `Account<T>` in solana-borsh mode
    // isn't zero-copy, so the write has to happen here explicitly.
    // `ctx.bumps.mint_authority` (`Context::bumps`, `naclac-core/src/context.rs`)
    // is populated in every mode, so this is a correct, portable no-op in
    // the two modes where the framework already wrote it automatically.
    ctx.accounts.mint_authority.bump = ctx.bumps.mint_authority;
    Ok(())
}
