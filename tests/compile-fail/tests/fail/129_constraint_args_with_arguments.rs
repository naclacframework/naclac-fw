#![allow(unexpected_cfgs)]
// `#[constraint_args]` takes no arguments.
use naclac_lang::prelude::*;

#[constraint_args(min)]
pub struct Args {
    pub min: Option<u64>,
}

fn main() {}
