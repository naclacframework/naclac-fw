#![allow(unexpected_cfgs)]
// A literal seed of exactly 32 bytes is valid.
use naclac_lang::prelude::*;

declare_id!("11111111111111111111111111111111");

#[derive(Accounts)]
pub struct Foo {
    #[account(seeds = [b"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"], bump)]
    pub account: Account<FooData>,
}

#[account]
pub struct FooData {
    pub value: u64,
}

fn main() {}
