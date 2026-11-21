#![allow(unexpected_cfgs)]
// The bare attribute is valid.
use naclac_lang::prelude::*;

#[constraint_args]
pub struct Args {
    pub min: Option<u64>,
}

fn main() {}
