#![allow(unexpected_cfgs)]
// A `ZcVec` in `#[instruction(..)]` names its element type.
use naclac_lang::prelude::*;

#[derive(Accounts)]
#[instruction(data: ZcVec)]
pub struct Foo {
    pub a: Signer,
}

fn main() {}
