#![allow(unexpected_cfgs)]
// An Accounts field is a framework wrapper type such as `Signer`.
use naclac_lang::prelude::*;

#[derive(Accounts)]
pub struct Foo {
    pub good: Signer,
}

fn main() {}
