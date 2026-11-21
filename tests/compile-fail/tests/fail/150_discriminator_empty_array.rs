#![allow(unexpected_cfgs)]
// A custom discriminator is 1 to 8 bytes long.
use naclac_lang::prelude::*;

#[event(discriminator = [])]
pub struct Logged {
    pub value: u64,
}

fn main() {}
