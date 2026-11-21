#![allow(unexpected_cfgs)]
// An integer at a fixed scale is valid.
use naclac_lang::prelude::*;

#[system]
pub fn compute(x: u64) -> Result<u64> {
    let _half = 5_000u64;
    Ok(x)
}

fn main() {}
