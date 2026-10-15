#![allow(unexpected_cfgs)]
// Valid counterpart to tests/fail/17_seeds_missing_bump.rs: an existing
// `Account<T>` with `seeds = [...]` and no `bump` discovers its bump on-chain.
use naclac_lang::prelude::*;

#[derive(Accounts)]
pub struct Foo {
    #[account(seeds = [b"foo"])]
    pub account: Account<FooData>,
}

#[component]
pub struct FooData {
    pub value: u64,
}

fn main() {}
