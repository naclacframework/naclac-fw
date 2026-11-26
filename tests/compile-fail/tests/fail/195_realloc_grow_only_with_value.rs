#![allow(unexpected_cfgs)]
// `realloc::grow_only` is a flag and takes no value.
use naclac_lang::prelude::*;

declare_id!("11111111111111111111111111111111");

#[derive(Accounts)]
#[instruction(new_space: u64)]
pub struct Foo {
    #[account(mut)]
    pub payer: Signer,

    /// SAFETY: `realloc::any_of` reads the raw account and checks its discriminator.
    #[account(mut, realloc::any_of = [FooData], realloc::payer = payer, realloc::grow_only = true)]
    pub account: UncheckedAccount,

    pub system_program: Program<System>,
}

#[account]
pub struct FooData {
    pub value: u64,
}

fn main() {}
