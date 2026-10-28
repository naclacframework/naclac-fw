//! Account creation and closing constraints.

use crate::components::{Entry, Growable, Ledger, Registry, Tiny, Vault};
use crate::constants::{SEED_ENTRY, SEED_GROW, SEED_LEDGER, SEED_REGISTRY};
use naclac_lang::prelude::*;

#[derive(Accounts)]
pub struct InitPlain {
    #[account(mut)]
    pub payer: Signer,
    #[account(
        init,
        payer = payer,
        space = 8 + core::mem::size_of::<Registry>(),
        seeds = [SEED_REGISTRY],
        bump
    )]
    pub registry: Account<Registry>,
    pub system_program: Program<System>,
}

pub fn init_plain(_ctx: Context<InitPlain>) -> Result {
    Ok(())
}

#[derive(Accounts)]
pub struct InitIfNeeded {
    #[account(mut)]
    pub payer: Signer,
    #[account(init_if_needed, payer = payer, seeds = [SEED_LEDGER], bump)]
    pub ledger: Account<Ledger>,
    pub system_program: Program<System>,
}

pub fn init_if_needed(_ctx: Context<InitIfNeeded>) -> Result {
    Ok(())
}

#[derive(Accounts)]
pub struct InitUnchecked {
    #[account(mut)]
    pub payer: Signer,
    #[account(
        init_unchecked,
        payer = payer,
        space = 8 + core::mem::size_of::<Tiny>(),
        seeds = [SEED_ENTRY],
        bump
    )]
    pub tiny: Account<Tiny>,
    pub system_program: Program<System>,
}

pub fn init_unchecked(_ctx: Context<InitUnchecked>) -> Result {
    Ok(())
}

/// Neither `beneficiary` nor `funder` nor `growable` is written `mut`: the `close`
/// destination, the `realloc::payer` and the reallocated account are writable anyway.
#[derive(Accounts)]
pub struct ImplicitMut {
    pub beneficiary: Signer,
    pub funder: Signer,
    #[account(mut, close = beneficiary)]
    pub vault: Account<Vault>,
    #[account(seeds = [SEED_GROW], bump, realloc = 64, realloc::payer = funder)]
    pub growable: Account<Growable>,
    pub system_program: Program<System>,
}

pub fn implicit_mut(_ctx: Context<ImplicitMut>) -> Result {
    Ok(())
}

#[derive(Accounts)]
pub struct CloseIt {
    #[account(mut)]
    pub payer: Signer,
    #[account(mut, close = payer)]
    pub vault: Account<Vault>,
    #[account(mut, close = payer)]
    pub entry: Account<Entry>,
}

pub fn close_it(_ctx: Context<CloseIt>) -> Result {
    Ok(())
}
