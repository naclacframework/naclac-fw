#![allow(unexpected_cfgs)]
// Error-code discriminants stay below the framework range.
use naclac_lang::prelude::*;

#[error_code]
pub enum Small {
    Fine,
    AlsoFine = 100,
}

fn main() {}
