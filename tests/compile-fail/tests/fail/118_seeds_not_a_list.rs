#![allow(unexpected_cfgs)]
// `seeds` takes a list or a `Type::seeds(..)` call.
use naclac_lang::prelude::*;

declare_id!("11111111111111111111111111111111");

#[derive(Accounts)]
pub struct Foo {
    #[account(seeds = 5, bump)]
    pub account: Account<FooData>,
}

#[account]
pub struct FooData {
    pub value: u64,
}

fn main() {}
