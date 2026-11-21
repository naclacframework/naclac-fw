#![allow(unexpected_cfgs)]
// Values of one decimal scale can be added.
use naclac_lang::prelude::*;

#[system(scale(a = 2, b = 2))]
pub fn compute(a: u64, b: u64) -> Result<u64> {
    let total = a + b;
    Ok(total)
}

fn main() {}
