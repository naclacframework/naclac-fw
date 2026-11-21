#![allow(unexpected_cfgs)]
// A `seeds` list within the limit is valid.
use naclac_lang::prelude::*;

declare_id!("11111111111111111111111111111111");

#[derive(Accounts)]
pub struct Foo {
    #[account(seeds = [b"a", b"b", b"c"], bump)]
    pub account: Account<FooData>,
}

#[account]
pub struct FooData {
    pub value: u64,
}

fn main() {}
