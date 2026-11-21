#![allow(unexpected_cfgs)]
// The `offset` is an integer literal.
use naclac_lang::prelude::*;

#[error_code(offset = "x")]
pub enum Failure {
    A,
}

fn main() {}
