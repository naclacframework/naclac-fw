#![allow(unexpected_cfgs)]
// A literal `offset` is valid.
use naclac_lang::prelude::*;

#[error_code(offset = 6000)]
pub enum Failure {
    A,
}

fn main() {}
