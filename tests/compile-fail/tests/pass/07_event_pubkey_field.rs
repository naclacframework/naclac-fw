#![allow(unexpected_cfgs)]
// An event field is typed `Address`.
use naclac_lang::prelude::*;

#[event]
pub struct FooEvent {
    pub good: Address,
}

fn main() {}
