#![allow(unexpected_cfgs)]
// An invariant that parses as a Rust boolean expression is valid.
use naclac_lang::prelude::*;

#[system(invariant = "result <= 100")]
pub fn foo(x: u64) -> Result<u64> {
    Ok(x)
}

fn main() {}
