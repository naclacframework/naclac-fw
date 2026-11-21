#![allow(unexpected_cfgs)]
// Every byte of a custom discriminator is an integer literal.
use naclac_lang::prelude::*;

#[event(discriminator = [FIRST, 2])]
pub struct Logged {
    pub value: u64,
}

fn main() {}
