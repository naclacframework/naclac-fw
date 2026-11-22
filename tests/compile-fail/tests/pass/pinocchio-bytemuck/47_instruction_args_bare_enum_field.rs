#![allow(unexpected_cfgs)]
// Under bytemuck instruction args carry an enum as its u8 tag.
use naclac_lang::prelude::*;

#[instruction_args]
pub struct Args {
    pub mode: u8,
    pub amount: u64,
}

fn main() {}
