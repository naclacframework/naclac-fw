#![allow(unexpected_cfgs)]
// A defined type name that is not a reserved IDL name.
use naclac_lang::prelude::*;

#[defined_type]
pub struct Settings {
    pub value: u8,
}

fn main() {}
