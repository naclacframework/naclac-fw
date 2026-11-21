#![allow(unexpected_cfgs)]
// A floating-point literal is rejected inside `#[system]`.
use naclac_lang::prelude::*;

#[system]
pub fn compute(x: u64) -> Result<u64> {
    let _half = 0.5;
    Ok(x)
}

fn main() {}
