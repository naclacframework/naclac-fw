#![allow(unexpected_cfgs)]
// Each of `dup` and `unsafe(alias)` is valid on its own field kind.
use naclac_lang::prelude::*;

#[derive(Accounts)]
pub struct Foo {
    pub a: Signer,
    /// SAFETY: test fixture.
    #[account(dup)]
    pub b: Signer,
    #[account(mut, unsafe(alias))]
    pub c: Signer,
}

fn main() {}
