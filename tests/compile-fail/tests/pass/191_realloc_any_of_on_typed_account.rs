#![allow(unexpected_cfgs)]
// `realloc::any_of` on an `UncheckedAccount` field is valid.
use naclac_lang::prelude::*;

declare_id!("11111111111111111111111111111111");

#[derive(Accounts)]
pub struct Foo {
    #[account(mut)]
    pub payer: Signer,

    /// SAFETY: `realloc::any_of` reads the raw account and checks its discriminator.
    #[account(mut, realloc::any_of = [FooData], realloc::payer = payer)]
    pub account: UncheckedAccount,

    pub system_program: Program<System>,
}

#[account]
pub struct FooData {
    pub value: u64,
}

fn main() {}
