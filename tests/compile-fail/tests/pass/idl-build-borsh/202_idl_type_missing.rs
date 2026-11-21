#![allow(unexpected_cfgs)]
// A field with an IDL type is valid.
use naclac_lang::prelude::*;

declare_id!("11111111111111111111111111111111");

#[account]
pub struct FooData {
    pub value: u8,
}

fn main() {}
