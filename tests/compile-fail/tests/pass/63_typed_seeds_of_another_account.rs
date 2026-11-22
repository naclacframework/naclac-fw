#![allow(unexpected_cfgs)]
// Typed seeds come from the account type of the field.
use naclac_lang::prelude::*;

declare_id!("11111111111111111111111111111111");

#[account]
#[seeds(b"first")]
pub struct First {
    pub value: u64,
}

#[derive(Accounts)]
pub struct Foo {
    #[account(seeds = First::seeds(), bump)]
    pub account: Account<First>,
}

fn main() {}
