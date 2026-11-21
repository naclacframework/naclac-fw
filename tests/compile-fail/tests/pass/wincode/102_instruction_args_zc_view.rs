// An owned `Vec<u8>` field is valid in an `#[instruction_args]` struct under Borsh and Wincode.
use naclac_lang::prelude::*;

#[instruction_args]
pub struct Args {
    pub data: Vec<u8>,
}

fn main() {}
