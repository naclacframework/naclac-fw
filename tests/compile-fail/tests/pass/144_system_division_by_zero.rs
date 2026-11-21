#![allow(unexpected_cfgs)]
// A division by a non-zero literal is valid.
use naclac_lang::prelude::*;

#[system]
pub fn compute(x: u64) -> Result<u64> {
    Ok(x / 2)
}

fn main() {}
