#![allow(unexpected_cfgs)]
// A known rounding mode is valid.
use naclac_lang::prelude::*;

#[system(rounding = "up")]
pub fn compute(x: u64) -> Result<u64> {
    Ok(x / 2)
}

fn main() {}
