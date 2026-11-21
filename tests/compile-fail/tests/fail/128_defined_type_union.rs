#![allow(unexpected_cfgs)]
// `#[defined_type]` applies to a struct or an enum.
use naclac_lang::prelude::*;

#[defined_type]
pub union Mixed {
    pub a: u8,
    pub b: u8,
}

fn main() {}
