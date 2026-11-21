#![allow(unexpected_cfgs)]

use naclac_lang::prelude::*;

#[defined_type] #[repr(u8)]
pub enum Mode {
    Off,
    On,
}

#[account]
pub struct Foo {
    pub mode: Mode,
}

fn main() {}
