#![allow(unexpected_cfgs)]
// A custom discriminator is an integer literal or an array of them.
use naclac_lang::prelude::*;

#[event(discriminator = "x")]
pub struct Logged {
    pub value: u64,
}

fn main() {}
