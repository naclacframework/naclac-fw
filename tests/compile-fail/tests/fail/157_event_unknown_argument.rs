#![allow(unexpected_cfgs)]
// An unknown event argument is rejected.
use naclac_lang::prelude::*;

#[event(bogus)]
pub struct Logged {
    pub value: u64,
}

fn main() {}
