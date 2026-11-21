#![allow(unexpected_cfgs)]
// `seeds::program` is a valid `seeds::` constraint.
use naclac_lang::prelude::*;

declare_id!("11111111111111111111111111111111");

#[derive(Accounts)]
pub struct Foo {
    #[account(seeds = [b"foo"], bump, seeds::program = naclac_lang::address!("11111111111111111111111111111111"))]
    pub account: Account<FooData>,
}

#[account]
pub struct FooData {
    pub value: u64,
}

fn main() {}
