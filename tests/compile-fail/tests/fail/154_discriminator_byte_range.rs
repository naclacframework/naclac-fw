#![allow(unexpected_cfgs)]
// A discriminator byte is in 0..=255.
use naclac_lang::prelude::*;

#[event(discriminator = [300])]
pub struct Logged {
    pub value: u64,
}

fn main() {}
