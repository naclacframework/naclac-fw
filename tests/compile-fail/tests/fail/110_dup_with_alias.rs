#![allow(unexpected_cfgs)]
// `dup` is for read-only fields and `unsafe(alias)` for `mut` ones, so they cannot be combined.
use naclac_lang::prelude::*;

#[derive(Accounts)]
pub struct Foo {
    pub a: Signer,
    /// SAFETY: test fixture.
    #[account(dup, unsafe(alias))]
    pub b: Signer,
}

fn main() {}
