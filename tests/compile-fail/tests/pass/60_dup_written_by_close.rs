#![allow(unexpected_cfgs)]
// A close destination is not a `dup` field.
use naclac_lang::prelude::*;

#[derive(Accounts)]
pub struct Foo {
    #[account(mut, close = b)]
    pub a: Account<Thing>,

    #[account(mut)]
    pub b: Signer,
}

#[account]
pub struct Thing {
    pub value: u64,
}

fn main() {}
