#![allow(unexpected_cfgs)]
// `unsafe(alias)` goes on a writable field.
use naclac_lang::prelude::*;

#[derive(Accounts)]
pub struct Foo {
    pub a: Signer,

    /// SAFETY: test fixture.
    #[account(mut, unsafe(alias))]
    pub b: Signer,
}

fn main() {}
