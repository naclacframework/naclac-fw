#![allow(unexpected_cfgs)]
// The `alloc` argument is written once.
use naclac_lang::prelude::*;

#[event(alloc, alloc)]
pub struct Logged {
    pub value: u64,
}

fn main() {}
