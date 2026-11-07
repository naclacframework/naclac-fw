#![allow(unexpected_cfgs)]
// Site: derive/src/system/fuzz.rs — `#[system(invariant = "...")]`
// whose string isn't a parseable Rust boolean expression.
use naclac_lang::prelude::*;

#[system(invariant = "result <=")]
pub fn foo(x: u64) -> Result<u64> {
    Ok(x)
}

fn main() {}