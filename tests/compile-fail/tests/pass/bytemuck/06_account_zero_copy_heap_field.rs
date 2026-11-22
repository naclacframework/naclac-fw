#![allow(unexpected_cfgs)]
// A zero-copy account stores fixed-size fields only.
use naclac_lang::prelude::*;

#[account]
pub struct Foo {
    pub good: [u8; 8],
}

fn main() {}
