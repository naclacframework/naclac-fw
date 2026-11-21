#![allow(unexpected_cfgs)]
// `error = "..."` names a Rust path.
use naclac_lang::prelude::*;

#[error_code]
pub enum MyError {
    A,
}

#[system(error = "not a path!")]
pub fn compute(x: u64) -> Result<u64> {
    Ok(x + 1)
}

fn main() {}
