#![allow(unexpected_cfgs)]
// A struct is valid.
use naclac_lang::prelude::*;

#[instruction_args]
pub struct Args {
    pub amount: u64,
}

fn main() {}
