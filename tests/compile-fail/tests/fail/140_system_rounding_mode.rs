#![allow(unexpected_cfgs)]
// A rounding mode is `"up"` or `"down"`.
use naclac_lang::prelude::*;

#[system(rounding = "sideways")]
pub fn compute(x: u64) -> Result<u64> {
    Ok(x / 2)
}

fn main() {}
