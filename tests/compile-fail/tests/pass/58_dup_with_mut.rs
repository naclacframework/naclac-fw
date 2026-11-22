#![allow(unexpected_cfgs)]
// `dup` stands alone on a field.
use naclac_lang::prelude::*;

#[derive(Accounts)]
pub struct Foo {
    pub a: Signer,

    /// SAFETY: test fixture.
    #[account(dup)]
    pub b: Signer,
}

fn main() {}
