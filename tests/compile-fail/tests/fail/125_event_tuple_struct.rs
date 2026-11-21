#![allow(unexpected_cfgs)]
// An `#[event]` needs named fields.
use naclac_lang::prelude::*;

#[event]
pub struct Logged(u64);

fn main() {}
