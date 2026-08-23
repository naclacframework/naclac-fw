#![allow(unexpected_cfgs)]
// Site: naclac-macros/src/instruction/init_cpi.rs — `mint::decimals =` on an
// `init` field with no `mint::authority =` alongside it. Previously a raw
// proc-macro panic ("proc macro panicked", no clean span); now a spanned
// `syn::Error`.
use naclac_lang::prelude::*;

#[derive(Accounts)]
pub struct Foo {
    #[account(mut)]
    pub payer: Signer,

    /// SAFETY: fully validated/constructed via `mint::decimals =`/`init`
    /// below — irrelevant here, this fixture never reaches that check.
    #[account(
        init,
        payer = payer,
        seeds = [b"mint"],
        bump = 255,
        mint::decimals = 6,
    )]
    pub mint: AccountInfo,

    pub token_program: Program<Token>,
    pub system_program: Program<System>,
}

fn main() {}
