#![allow(unexpected_cfgs)]
// A known argument is valid.
use naclac_lang::prelude::*;

#[event(discriminator = 7)]
pub struct Logged {
    pub value: u64,
}

fn main() {}
