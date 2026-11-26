#![allow(unexpected_cfgs)]
// A mint with decimals also names its authority.
use naclac_lang::prelude::*;

declare_id!("11111111111111111111111111111111");

#[derive(Accounts)]
pub struct Foo {
    #[account(mut)]
    pub payer: Signer,

    /// SAFETY: only used as the mint's `authority`; never read or deserialized.
    pub authority: UncheckedAccount,

    #[account(init, payer = payer, seeds = [b"mint"], bump, mint::decimals = 6, mint::authority = authority)]
    pub mint: Account<Mint>,

    pub token_program: Program<Token>,
    pub system_program: Program<System>,
}

fn main() {}
