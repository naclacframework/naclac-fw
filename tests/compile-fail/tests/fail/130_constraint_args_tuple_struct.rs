#![allow(unexpected_cfgs)]
// `#[constraint_args]` needs named fields, one per key.
use naclac_lang::prelude::*;

#[constraint_args]
pub struct Args(Option<u64>);

fn main() {}
