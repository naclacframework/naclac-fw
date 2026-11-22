#![allow(unexpected_cfgs)]
// An account field is typed `Address`.
use naclac_lang::prelude::*;

declare_id!("11111111111111111111111111111111");

#[account]
pub struct Foo {
    pub good: Address,
}

fn main() {}
