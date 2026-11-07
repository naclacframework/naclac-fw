#![allow(unexpected_cfgs)]
// Site: derive/src/event/mod.rs — `#[event]` field typed `Pubkey`
// (deprecated in favor of `Address`).
use naclac_lang::prelude::*;

#[event]
pub struct FooEvent {
    pub bad: Pubkey,
}

fn main() {}