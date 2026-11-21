#![allow(unexpected_cfgs)]
// One `discriminator` is valid.
use naclac_lang::prelude::*;

#[event(discriminator = 1)]
pub struct Logged {
    pub value: u64,
}

fn main() {}
