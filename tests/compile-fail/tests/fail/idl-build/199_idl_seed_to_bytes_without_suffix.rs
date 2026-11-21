#![allow(unexpected_cfgs)]
// Under `idl-build` a `.to_le_bytes()` literal seed needs a type suffix.
use naclac_lang::prelude::*;

declare_id!("11111111111111111111111111111111");

#[derive(Accounts)]
pub struct Foo {
    #[account(seeds = [b"a", &0.to_le_bytes()], bump)]
    pub account: Account<FooData>,
}

#[account]
pub struct FooData {
    pub value: u64,
}

fn main() {}
