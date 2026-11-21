#![allow(unexpected_cfgs)]
// A known option is valid.
use naclac_lang::prelude::*;

#[system(no_rewrite)]
pub fn compute(x: u64) -> Result<u64> {
    Ok(x)
}

fn main() {}
