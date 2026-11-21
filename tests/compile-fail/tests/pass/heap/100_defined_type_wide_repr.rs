// The same enum without the wide `repr` is valid in the Borsh and Wincode layouts.
use naclac_lang::prelude::*;

#[defined_type]
pub enum Level {
    Low = 1,
    High = 2,
}

fn main() {}
