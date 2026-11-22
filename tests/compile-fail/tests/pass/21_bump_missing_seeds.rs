#![allow(unexpected_cfgs)]
// A bump goes with the seeds it verifies.
use naclac_lang::prelude::*;

declare_id!("11111111111111111111111111111111");

#[derive(Accounts)]
pub struct Foo {
    #[account(seeds = [b"foo"], bump)]
    pub account: Account<FooData>,
}

#[account]
pub struct FooData {
    pub value: u64,
}

fn main() {}
