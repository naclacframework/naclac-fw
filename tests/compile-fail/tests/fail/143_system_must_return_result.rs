#![allow(unexpected_cfgs)]
// A `#[system]` function must return `Result<T>`.
use naclac_lang::prelude::*;

#[system]
pub fn compute(x: u64) -> u64 {
    x
}

fn main() {}
