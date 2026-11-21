// A plain `#[event]` carries a `String` field under Borsh and Wincode.
use naclac_lang::prelude::*;

#[event]
pub struct Logged {
    pub note: String,
}

fn main() {}
