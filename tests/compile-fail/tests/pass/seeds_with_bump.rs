#![allow(unexpected_cfgs)]
// Bare `bump` on an existing `Account<T>` whose account declares a `bump`
// field reads the stored bump.
use naclac_lang::prelude::*;

#[derive(Accounts)]
pub struct Foo {
    #[account(seeds = [b"foo"], bump)]
    pub account: Account<FooData>,
}

#[account]
pub struct FooData {
    pub bump: u8,
    pub value: u64,
}

fn main() {}