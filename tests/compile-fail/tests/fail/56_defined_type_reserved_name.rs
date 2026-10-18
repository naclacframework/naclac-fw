#![allow(unexpected_cfgs)]
// Site: macros/src/lib.rs — a `#[defined_type]` named `Bool`, the name
// naclac's IDL and generated clients give to its one-byte boolean.
use naclac_lang::prelude::*;

#[defined_type]
pub struct Bool {
    pub value: u8,
}

fn main() {}
