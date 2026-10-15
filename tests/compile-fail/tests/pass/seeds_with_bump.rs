#![allow(unexpected_cfgs)]
// Bare `bump` on an existing `Account<T>` whose component declares a `bump`
// field reads the stored bump.
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