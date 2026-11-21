#![allow(unexpected_cfgs)]
// A user error code starts at 6000, so a lower `offset` is rejected.
use naclac_lang::prelude::*;

#[error_code(offset = 5000)]
pub enum Low {
    A,
}

fn main() {}
