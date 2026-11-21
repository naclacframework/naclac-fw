#![allow(unexpected_cfgs)]
// An associated token account is created by the Associated Token Program, so `init_unchecked` is rejected.
use naclac_lang::prelude::*;

declare_id!("11111111111111111111111111111111");

#[derive(Accounts)]
pub struct Foo {
    #[account(mut)]
    pub payer: Signer,

    pub mint: Account<Mint>,

    /// SAFETY: only used as the ATA's `authority`; never read or deserialized.
    pub owner: AccountInfo,

    #[account(init_unchecked, payer = payer, associated_token::mint = mint, associated_token::authority = owner)]
    pub vault: Account<TokenAccount>,

    pub token_program: Program<Token>,
    pub associated_token_program: Program<AssociatedToken>,
    pub system_program: Program<System>,
}

fn main() {}
