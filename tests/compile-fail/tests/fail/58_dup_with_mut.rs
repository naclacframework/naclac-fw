#![allow(unexpected_cfgs)]
// Site: derive/src/accounts/field/parser/validate.rs — a `dup` field is
// read-only, so `mut` on it is rejected.
use naclac_lang::prelude::*;

#[derive(Accounts)]
pub struct Foo {
    pub a: Signer,
    /// SAFETY: test fixture.
    #[account(dup, mut)]
    pub b: Signer,
}

fn main() {}
