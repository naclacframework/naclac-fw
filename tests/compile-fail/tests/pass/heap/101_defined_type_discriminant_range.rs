// Discriminants up to 255 are valid in the Borsh and Wincode layouts.
use naclac_lang::prelude::*;

#[defined_type]
pub enum Big {
    Small = 1,
    Large = 255,
}

fn main() {}
