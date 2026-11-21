#![allow(unexpected_cfgs)]
// A `#[defined_type]` has an IDL type.
use naclac_lang::prelude::*;

declare_id!("11111111111111111111111111111111");

#[defined_type]
pub struct Level {
    pub value: u8,
}

#[account]
pub struct FooData {
    pub level: Level,
}

fn main() {}
