#![allow(unexpected_cfgs)]

use naclac_lang::prelude::*;

#[defined_type]
pub enum Mode {
    Off,
    On,
}

#[component]
pub struct Foo {
    pub mode: Mode,
}

fn main() {}
