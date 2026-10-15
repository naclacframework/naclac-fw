#![allow(unexpected_cfgs)]
// An existing associated token account with `associated_token::mint`/
// `associated_token::authority` and no `associated_token::bump`: the ATA
// bump is discovered on-chain after the token-program owner check.
use naclac_lang::prelude::*;

#[derive(Accounts)]
pub struct Foo {
    pub mint: Account<Mint>,

    /// SAFETY: only used as the ATA's `authority` for constraint
    /// verification below; never read or deserialized.
    pub owner: AccountInfo,

    #[account(
        associated_token::mint = mint,
        associated_token::authority = owner,
    )]
    pub associated_token: Account<TokenAccount>,

    pub token_program: Program<Token>,
}

fn main() {}
