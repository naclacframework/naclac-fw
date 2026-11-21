#![allow(unexpected_cfgs)]
// The largest byte is valid.
use naclac_lang::prelude::*;

#[event(discriminator = [255])]
pub struct Logged {
    pub value: u64,
}

fn main() {}
