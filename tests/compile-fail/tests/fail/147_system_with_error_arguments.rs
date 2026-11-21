#![allow(unexpected_cfgs)]
// `with_error!` takes a path and an expression.
use naclac_lang::prelude::*;

#[error_code]
pub enum MyError {
    A,
}

#[system]
pub fn compute(x: u64) -> Result<u64> {
    Ok(with_error!(x))
}

fn main() {}
