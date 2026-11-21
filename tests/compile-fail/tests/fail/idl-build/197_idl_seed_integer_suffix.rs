#![allow(unexpected_cfgs)]
// Under `idl-build` a single-byte integer seed has no suffix or a `u8` suffix.
use naclac_lang::prelude::*;

declare_id!("11111111111111111111111111111111");

#[derive(Accounts)]
pub struct Foo {
    #[account(seeds = [b"a", &[5u16]], bump)]
    pub account: Account<FooData>,
}

#[account]
pub struct FooData {
    pub value: u64,
}

fn main() {}
