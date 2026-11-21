#![allow(unexpected_cfgs)]
// `realloc::any_of` reads the raw account, so it needs an `AccountInfo` field.
use naclac_lang::prelude::*;

declare_id!("11111111111111111111111111111111");

#[derive(Accounts)]
pub struct Foo {
    #[account(mut)]
    pub payer: Signer,

    #[account(mut, realloc::any_of = [FooData], realloc::payer = payer)]
    pub account: Account<FooData>,

    pub system_program: Program<System>,
}

#[account]
pub struct FooData {
    pub value: u64,
}

fn main() {}
