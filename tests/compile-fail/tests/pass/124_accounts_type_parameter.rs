#![allow(unexpected_cfgs)]
// The same struct without a type parameter is valid.
use naclac_lang::prelude::*;

#[derive(Accounts)]
pub struct Foo {
    pub a: Signer,
}

fn main() {}
