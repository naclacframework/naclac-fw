#![allow(unexpected_cfgs)]
// A #[system] function works in integers and returns a Result.
use naclac_lang::prelude::*;

#[system]
pub fn foo(x: u64) -> Result<u64> {
    Ok(x)
}

fn main() {}
