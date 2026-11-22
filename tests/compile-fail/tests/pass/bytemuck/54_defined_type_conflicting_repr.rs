#![allow(unexpected_cfgs)]
// A defined type takes no `repr` of its own.
use naclac_lang::prelude::*;

#[defined_type]
pub struct Settings {
    pub a: u8,
}

fn main() {}
