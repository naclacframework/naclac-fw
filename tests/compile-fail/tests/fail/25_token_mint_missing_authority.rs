#![allow(unexpected_cfgs)]
// Site: naclac-macros/src/instruction/init_cpi.rs — `token::mint =` on an
// `init` field with no `token::authority =` alongside it. Previously a raw
// proc-macro panic; now a spanned `syn::Error`.
use naclac_lang::prelude::*;

#[derive(Accounts)]
pub struct Foo {
    #[account(mut)]
    pub payer: Signer,

    pub mint: Account<Mint>,

    /// SAFETY: fully validated/constructed via `token::mint =`/`init`
    /// below — irrelevant here, this fixture never reaches that check.
    #[account(
        init,
        payer = payer,
        seeds = [b"vault"],
        bump = 255,
        token::mint = mint,
    )]
    pub vault: AccountInfo,

    pub token_program: Program<Token>,
    pub system_program: Program<System>,
}

fn main() {}
