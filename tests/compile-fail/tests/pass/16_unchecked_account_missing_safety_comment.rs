#![allow(unexpected_cfgs)]
// A bare UncheckedAccount field carries a SAFETY doc comment.
use naclac_lang::prelude::*;

#[derive(Accounts)]
pub struct Foo {
    /// SAFETY: only its address is used; never read or deserialized.
    pub unchecked: UncheckedAccount,
}

fn main() {}
