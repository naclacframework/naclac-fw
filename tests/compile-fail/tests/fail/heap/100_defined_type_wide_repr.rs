// A Borsh or Wincode enum tag is one byte, so a wider `repr` is rejected.
use naclac_lang::prelude::*;

#[defined_type]
#[repr(u16)]
pub enum Level {
    Low = 1,
    High = 2,
}

fn main() {}
