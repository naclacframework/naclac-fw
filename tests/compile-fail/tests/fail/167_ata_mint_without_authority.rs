#![allow(unexpected_cfgs)]
// An existing associated token account names its authority with its mint.
use naclac_lang::prelude::*;

declare_id!("11111111111111111111111111111111");

#[derive(Accounts)]
pub struct Foo {
    #[account(mut)]
    pub payer: Signer,

    pub mint: Account<Mint>,

    /// SAFETY: only used as the ATA's `authority`; never read or deserialized.
    pub owner: UncheckedAccount,

    #[account(associated_token::mint = mint)]
    pub vault: Account<TokenAccount>,

    pub token_program: Program<Token>,
    pub associated_token_program: Program<AssociatedToken>,
    pub system_program: Program<System>,
}

fn main() {}
