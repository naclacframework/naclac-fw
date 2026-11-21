// A Borsh or Wincode enum tag is one byte, so a discriminant above 255 is rejected.
use naclac_lang::prelude::*;

#[defined_type]
pub enum Big {
    Small = 1,
    Large = 300,
}

fn main() {}
