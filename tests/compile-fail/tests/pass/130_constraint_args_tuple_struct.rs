#![allow(unexpected_cfgs)]
// A struct with named fields is valid.
use naclac_lang::prelude::*;

#[constraint_args]
pub struct Args {
    pub min: Option<u64>,
}

fn main() {}
