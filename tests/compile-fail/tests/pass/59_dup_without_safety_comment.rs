#![allow(unexpected_cfgs)]
// A `dup` field carries a SAFETY doc.
use naclac_lang::prelude::*;

#[derive(Accounts)]
pub struct Foo {
    pub a: Signer,

    /// SAFETY: test fixture.
    #[account(dup)]
    pub b: Signer,
}

fn main() {}
