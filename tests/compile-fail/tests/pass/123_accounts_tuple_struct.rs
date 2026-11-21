#![allow(unexpected_cfgs)]
// A struct with named fields is valid.
use naclac_lang::prelude::*;

#[derive(Accounts)]
pub struct Foo {
    pub a: Signer,
}

fn main() {}
