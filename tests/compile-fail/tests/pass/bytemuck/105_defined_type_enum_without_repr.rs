// The same enum with an explicit `repr` is valid in the zero-copy layout.
use naclac_lang::prelude::*;

#[defined_type]
#[repr(u8)]
pub enum Mode {
    Off,
    On,
}

fn main() {}
