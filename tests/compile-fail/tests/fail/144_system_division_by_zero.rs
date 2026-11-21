#![allow(unexpected_cfgs)]
// A division by a literal zero is rejected at compile time.
use naclac_lang::prelude::*;

#[system]
pub fn compute(x: u64) -> Result<u64> {
    Ok(x / 0)
}

fn main() {}
