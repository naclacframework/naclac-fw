#![allow(unexpected_cfgs)]
// A scale in range is valid.
use naclac_lang::prelude::*;

#[system(scale(x = 2))]
pub fn compute(x: u64) -> Result<u64> {
    Ok(x)
}

fn main() {}
