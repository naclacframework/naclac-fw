// Valid counterpart of fail/44: the highest discriminant whose user error
// code (6000 + 3999 = 9999) stays below the framework range.
use naclac_lang::prelude::*;

#[error_code]
pub enum AtLimit {
    Fine,
    Highest = 3999,
}

fn main() {}
