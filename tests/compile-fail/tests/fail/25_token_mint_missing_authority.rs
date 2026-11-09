#![allow(unexpected_cfgs)]
// Site: derive/src/accounts/field/init_cpi — `token::mint =` on an
// `init` field with no `token::authority =` alongside it. Previously a raw
// proc-macro panic; now a spanned `syn::Error`.
use naclac_lang::prelude::*;

declare_id!("6queAeH2Cq6WPuq6tpgRFBtFBanSNW3zdc9Y8dm4g7oF");

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
        bump,
        token::mint = mint,
    )]
    pub vault: AccountInfo,

    pub token_program: Program<Token>,
    pub system_program: Program<System>,
}

fn main() {}
