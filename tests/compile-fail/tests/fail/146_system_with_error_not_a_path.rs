#![allow(unexpected_cfgs)]
// `with_error!` names a Rust path as its first argument.
use naclac_lang::prelude::*;

#[error_code]
pub enum MyError {
    A,
}

#[system]
pub fn compute(x: u64) -> Result<u64> {
    Ok(with_error!("not a path!", x + 1))
}

fn main() {}
