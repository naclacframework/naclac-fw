// `#[event(alloc)]` is the zero-copy layout's event form; Borsh and Wincode use plain `#[event]`.
use naclac_lang::prelude::*;

#[event(alloc)]
pub struct Logged {
    pub note: String,
}

fn main() {}
