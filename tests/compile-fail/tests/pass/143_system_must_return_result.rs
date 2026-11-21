#![allow(unexpected_cfgs)]
// A function returning `Result<T>` is valid.
use naclac_lang::prelude::*;

#[system]
pub fn compute(x: u64) -> Result<u64> {
    Ok(x)
}

fn main() {}
