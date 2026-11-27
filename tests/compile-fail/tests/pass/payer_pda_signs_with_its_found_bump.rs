#![allow(unexpected_cfgs)]
// A PDA payer with constant seeds signs the ATA creation with the bump the compiler found.
use naclac_lang::prelude::*;

declare_id!("11111111111111111111111111111111");

#[derive(Accounts)]
pub struct Foo {
    /// SAFETY: a PDA that pays for the account and signs through its seeds.
    #[account(mut, seeds = [b"payer"], bump)]
    pub payer: UncheckedAccount,

    pub mint: Account<Mint>,

    /// SAFETY: only used as the ATA's `authority`; never read or deserialized.
    pub owner: UncheckedAccount,

    #[account(init_if_needed, payer = payer, associated_token::mint = mint, associated_token::authority = owner, extra_accounts = [owner])]
    pub vault: Account<TokenAccount>,

    pub token_program: Program<Token>,
    pub associated_token_program: Program<AssociatedToken>,
    pub system_program: Program<System>,
}

fn main() {}
