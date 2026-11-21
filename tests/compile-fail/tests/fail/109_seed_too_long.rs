#![allow(unexpected_cfgs)]
// A PDA seed is at most 32 bytes, so a longer literal seed is rejected.
use naclac_lang::prelude::*;

declare_id!("11111111111111111111111111111111");

#[derive(Accounts)]
pub struct Foo {
    #[account(seeds = [b"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"], bump)]
    pub account: Account<FooData>,
}

#[account]
pub struct FooData {
    pub value: u64,
}

fn main() {}
