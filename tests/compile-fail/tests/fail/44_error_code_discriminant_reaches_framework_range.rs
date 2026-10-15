// Site: macros/src/error_code.rs — a user error code
// (6000 + discriminant) must stay below 10000, where framework error codes
// begin.
use naclac_lang::prelude::*;

#[error_code]
pub enum TooLarge {
    Fine,
    Collides = 4000,
}

fn main() {}
