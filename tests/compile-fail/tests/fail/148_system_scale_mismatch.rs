#![allow(unexpected_cfgs)]
// Adding values of different decimal scales is rejected.
use naclac_lang::prelude::*;

#[system(scale(a = 2, b = 3))]
pub fn compute(a: u64, b: u64) -> Result<u64> {
    Ok(a + b)
}

fn main() {}
