#![allow(unexpected_cfgs)]
// A constant expression that overflows is rejected at compile time.
use naclac_lang::prelude::*;

#[system]
pub fn compute() -> Result<u128> {
    Ok(340282366920938463463374607431768211455u128 + 1)
}

fn main() {}
