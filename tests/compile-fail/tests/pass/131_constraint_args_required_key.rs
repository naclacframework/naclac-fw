#![allow(unexpected_cfgs)]
// An optional key is valid.
use naclac_lang::prelude::*;

#[constraint_args]
pub struct Args {
    pub min: Option<u64>,
}

fn main() {}
