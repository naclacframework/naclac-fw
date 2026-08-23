#![allow(unexpected_cfgs)]
// Site: naclac-macros/src/instruction/init_cpi.rs — an `init` field with
// `associated_token::mint =`/`associated_token::authority =` but no
// `associated_token_program: Program<AssociatedToken>` anywhere in the
// struct. Previously a raw proc-macro panic; now a spanned `syn::Error`.
use naclac_lang::prelude::*;

#[derive(Accounts)]
pub struct Foo {
    #[account(mut)]
    pub payer: Signer,

    pub mint: Account<Mint>,

    /// SAFETY: only used as the ATA's `authority` for constraint
    /// verification below; never read or deserialized.
    pub owner: AccountInfo,

    /// SAFETY: fully validated/constructed via `associated_token::mint =`/
    /// `init` below — irrelevant here, this fixture never reaches that check.
    #[account(
        init,
        payer = payer,
        associated_token::mint = mint,
        associated_token::authority = owner,
    )]
    pub vault: AccountInfo,

    pub token_program: Program<Token>,
    pub system_program: Program<System>,
}

fn main() {}
