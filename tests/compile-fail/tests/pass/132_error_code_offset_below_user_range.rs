#![allow(unexpected_cfgs)]
// An `offset` at or above 6000 is valid.
use naclac_lang::prelude::*;

#[error_code(offset = 6000)]
pub enum Low {
    A,
}

fn main() {}
