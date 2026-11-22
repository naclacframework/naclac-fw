#![allow(unexpected_cfgs)]
// An alloc event under bytemuck holds fixed-size fields; an enum is stored as its u8 tag.
use naclac_lang::prelude::*;

#[event(alloc)]
pub struct Log {
    pub head: u8,
    pub mode: u8,
}

fn main() {}
