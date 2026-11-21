#![allow(unexpected_cfgs)]
// An event with named fields is valid.
use naclac_lang::prelude::*;

#[event]
pub struct Logged {
    pub value: u64,
}

fn main() {}
