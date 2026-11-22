#![allow(unexpected_cfgs)]
// An alloc event holds plain fixed-size fields.
use naclac_lang::prelude::*;

#[event(alloc)]
pub struct Logged {
    pub id: u64,
    pub note: u8,
}

fn main() {}
