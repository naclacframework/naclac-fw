#![allow(unexpected_cfgs)]
// `#[instruction_args]` applies to a struct.
use naclac_lang::prelude::*;

#[instruction_args]
pub enum Args {
    A,
}

fn main() {}
