#![allow(unexpected_cfgs)]
// A token account with a mint also names its authority.
use naclac_lang::prelude::*;

declare_id!("11111111111111111111111111111111");

#[derive(Accounts)]
pub struct Foo {
    #[account(mut)]
    pub payer: Signer,

    pub mint: Account<Mint>,

    /// SAFETY: only used as the token account's `authority`; never read or deserialized.
    pub owner: AccountInfo,

    #[account(init, payer = payer, seeds = [b"vault"], bump, token::mint = mint, token::authority = owner)]
    pub vault: Account<TokenAccount>,

    pub token_program: Program<Token>,
    pub system_program: Program<System>,
}

fn main() {}
