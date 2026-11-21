#![allow(unexpected_cfgs)]
// A path is valid.
use naclac_lang::prelude::*;

#[error_code]
pub enum MyError {
    A,
}

#[system(error = "MyError::A")]
pub fn compute(x: u64) -> Result<u64> {
    Ok(x + 1)
}

fn main() {}
