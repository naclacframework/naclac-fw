#![allow(unexpected_cfgs)]
// An unknown `#[system]` option is rejected.
use naclac_lang::prelude::*;

#[system(bogus)]
pub fn compute(x: u64) -> Result<u64> {
    Ok(x)
}

fn main() {}
