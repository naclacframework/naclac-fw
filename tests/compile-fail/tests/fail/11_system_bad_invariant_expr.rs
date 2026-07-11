#![allow(unexpected_cfgs)]
// Site: naclac-macros/src/system.rs:705 — `#[system(invariant = "...")]`
// whose string isn't a parseable Rust boolean expression.
use naclac_lang::prelude::*;

#[system(invariant = "result <=")]
pub fn foo(x: u64) -> Result<u64> {
    Ok(x)
}

fn main() {}