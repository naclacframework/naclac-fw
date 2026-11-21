// A `ZcVec` field is a view into the instruction data, which Borsh and Wincode cannot encode.
use naclac_lang::prelude::*;

#[instruction_args]
pub struct Args {
    pub data: ZcVec<u8>,
}

fn main() {}
