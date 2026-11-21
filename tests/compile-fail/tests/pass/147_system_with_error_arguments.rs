#![allow(unexpected_cfgs)]
// The two-argument form is valid.
use naclac_lang::prelude::*;

#[error_code]
pub enum MyError {
    A,
}

#[system]
pub fn compute(x: u64) -> Result<u64> {
    Ok(with_error!("MyError::A", x + 1))
}

fn main() {}
