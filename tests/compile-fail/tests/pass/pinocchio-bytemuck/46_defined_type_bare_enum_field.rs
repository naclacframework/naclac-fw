#![allow(unexpected_cfgs)]
// Under bytemuck a defined type stores an enum as its u8 tag.
use naclac_lang::prelude::*;

#[defined_type]
pub struct Settings {
    pub mode: u8,
}

fn main() {}
