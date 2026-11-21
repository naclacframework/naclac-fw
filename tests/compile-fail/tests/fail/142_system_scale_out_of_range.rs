#![allow(unexpected_cfgs)]
// A decimal scale is an integer in 0..=255.
use naclac_lang::prelude::*;

#[system(scale(x = 300))]
pub fn compute(x: u64) -> Result<u64> {
    Ok(x)
}

fn main() {}
