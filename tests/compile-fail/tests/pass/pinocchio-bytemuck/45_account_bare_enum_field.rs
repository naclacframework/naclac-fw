#![allow(unexpected_cfgs)]
// Under bytemuck an account stores an enum as its u8 tag.
use naclac_lang::prelude::*;

#[account]
pub struct Foo {
    pub mode: u8,
}

fn main() {}
