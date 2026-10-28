//! `realloc` in every form: an expression, `zero`, and the discriminator-driven `any_of`.

use crate::components::{Growable, Registry, Vault};
use crate::constants::SEED_GROW;
use naclac_lang::prelude::*;

#[derive(Accounts)]
#[instruction(new_space: u64)]
pub struct ReallocExpr {
    #[account(mut)]
    pub payer: Signer,
    #[account(
        mut,
        seeds = [SEED_GROW],
        bump,
        realloc = new_space as usize,
        realloc::payer = payer,
        realloc::zero = true
    )]
    pub growable: Account<Growable>,
    pub system_program: Program<System>,
}

pub fn realloc_expr(_ctx: Context<ReallocExpr>, _new_space: u64) -> Result {
    Ok(())
}

#[derive(Accounts)]
pub struct ReallocAnyOf {
    /// SAFETY: its size follows whichever listed component its discriminator names.
    #[account(
        mut,
        owner = crate::ID,
        realloc::any_of = [Vault, Registry],
        realloc::grow_only = true,
        realloc::payer = payer,
    )]
    pub target: AccountInfo,
    #[account(mut)]
    pub payer: Signer,
    pub system_program: Program<System>,
}

pub fn realloc_any_of(_ctx: Context<ReallocAnyOf>) -> Result {
    Ok(())
}
