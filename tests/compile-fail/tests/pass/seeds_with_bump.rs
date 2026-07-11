#![allow(unexpected_cfgs)]
// Valid counterpart to tests/fail/17_seeds_missing_bump.rs — bare `bump`
// satisfies the "seeds requires bump" check.
use naclac_lang::prelude::*;

#[derive(Accounts)]
pub struct Foo {
    #[account(seeds = [b"foo"], bump)]
    pub account: Account<FooData>,
}

#[component]
pub struct FooData {
    pub bump: u8,
    pub value: u64,
}

fn main() {}