#![allow(unexpected_cfgs)]
// The `discriminator` argument is written once.
use naclac_lang::prelude::*;

#[event(discriminator = 1, discriminator = 2)]
pub struct Logged {
    pub value: u64,
}

fn main() {}
