#![allow(unexpected_cfgs)]
// Every constraint key is an `Option`, so it can be left out.
use naclac_lang::prelude::*;

#[constraint_args]
pub struct Args {
    pub min: u64,
}

fn main() {}
