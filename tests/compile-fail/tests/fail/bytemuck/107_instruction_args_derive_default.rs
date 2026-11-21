// The zero-copy layout generates `Default` for a fixed-field `#[instruction_args]`, so a user derive conflicts.
use naclac_lang::prelude::*;

#[instruction_args]
#[derive(Default)]
pub struct Args {
    pub amount: u64,
}

fn main() {}
