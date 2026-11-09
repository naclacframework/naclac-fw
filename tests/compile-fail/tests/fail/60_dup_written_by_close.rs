#![allow(unexpected_cfgs)]
// Site: derive/src/accounts/field/parser/mod.rs — `close = b` makes `b`
// mutable, which a `dup` field cannot be.
use naclac_lang::prelude::*;

#[derive(Accounts)]
pub struct Foo {
    #[account(close = b)]
    pub a: Account<Thing>,
    /// SAFETY: test fixture.
    #[account(dup)]
    pub b: AccountInfo,
}

#[account]
pub struct Thing {
    pub value: u64,
}

fn main() {}
