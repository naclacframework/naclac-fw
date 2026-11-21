#![allow(unexpected_cfgs)]
// A struct is valid.
use naclac_lang::prelude::*;

#[defined_type]
pub struct Mixed {
    pub a: u8,
}

fn main() {}
