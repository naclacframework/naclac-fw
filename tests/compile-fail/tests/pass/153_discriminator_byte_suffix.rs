#![allow(unexpected_cfgs)]
// A `u8` suffix is valid.
use naclac_lang::prelude::*;

#[event(discriminator = [1u8])]
pub struct Logged {
    pub value: u64,
}

fn main() {}
