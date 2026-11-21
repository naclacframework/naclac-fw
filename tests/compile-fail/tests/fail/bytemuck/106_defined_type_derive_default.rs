// The zero-copy layout generates `Default` for a `#[defined_type]`, so a user derive conflicts.
use naclac_lang::prelude::*;

#[defined_type]
#[derive(Default)]
pub struct Settings {
    pub a: u8,
}

fn main() {}
