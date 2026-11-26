#![allow(unexpected_cfgs)]
// `realloc::zero` does not exist: realloc always zeroes the bytes it grows.
use naclac_lang::prelude::*;

declare_id!("11111111111111111111111111111111");

#[derive(Accounts)]
#[instruction(new_space: u64)]
pub struct Foo {
    #[account(mut)]
    pub payer: Signer,

    /// SAFETY: `realloc::any_of` reads the raw account and checks its discriminator.
    #[account(mut, realloc::any_of = [FooData], realloc::payer = payer, realloc::zero = true)]
    pub account: UncheckedAccount,

    pub system_program: Program<System>,
}

#[account]
pub struct FooData {
    pub value: u64,
}

fn main() {}
