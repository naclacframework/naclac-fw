#![allow(unexpected_cfgs)]
// A discriminator byte has no suffix or a `u8` suffix.
use naclac_lang::prelude::*;

#[event(discriminator = [1u16])]
pub struct Logged {
    pub value: u64,
}

fn main() {}
