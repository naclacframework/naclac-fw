#![allow(unexpected_cfgs)]
// Under bytemuck an event carries an enum as its u8 tag.
use naclac_lang::prelude::*;

#[event]
pub struct Log {
    pub head: u8,
    pub mode: u8,
}

fn main() {}
