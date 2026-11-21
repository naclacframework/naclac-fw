#![allow(unexpected_cfgs)]
// A scale mismatch is rejected in a returned value, not only in a `let`.
use naclac_lang::prelude::*;

#[system(scale(a = 2, b = 3))]
pub fn compute(a: u64, b: u64) -> Result<u64> {
    Ok(a + b)
}

fn main() {}
