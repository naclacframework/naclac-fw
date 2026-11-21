#![allow(unexpected_cfgs)]
// Integer literals are valid.
use naclac_lang::prelude::*;

#[event(discriminator = [1, 2])]
pub struct Logged {
    pub value: u64,
}

fn main() {}
