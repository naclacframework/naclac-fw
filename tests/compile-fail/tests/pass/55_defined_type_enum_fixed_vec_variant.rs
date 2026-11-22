#![allow(unexpected_cfgs)]
// An enum variant holds plain fixed-size fields.
use naclac_lang::prelude::*;

#[defined_type]
#[repr(u8)]
pub enum Action {
    Push { item: u8 },
}

fn main() {}
