#![allow(unexpected_cfgs)]
// A `ZcVec<u8>` is valid.
use naclac_lang::prelude::*;

#[derive(Accounts)]
#[instruction(data: ZcVec<u8>)]
pub struct Foo {
    pub a: Signer,
}

fn main() {}
