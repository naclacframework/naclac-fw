#![allow(unexpected_cfgs)]
// A constant expression that fits is valid.
use naclac_lang::prelude::*;

#[system]
pub fn compute() -> Result<u128> {
    Ok(1u128 + 1)
}

fn main() {}
