#![allow(unexpected_cfgs)]
// `#[instruction_args]` needs named fields.
use naclac_lang::prelude::*;

#[instruction_args]
pub struct Args(u64);

fn main() {}
