#![allow(unexpected_cfgs)]
// #[error_code] takes only an `offset` argument.
use naclac_lang::prelude::*;

#[error_code(bogus = 1)]
pub enum Failure {
    A,
}

fn main() {}
