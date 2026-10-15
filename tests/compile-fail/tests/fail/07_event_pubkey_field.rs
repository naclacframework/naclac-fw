#![allow(unexpected_cfgs)]
// Site: naclac-macros/src/event.rs:29 — `#[event]` field typed `Pubkey`
// (deprecated in favor of `Address`).
use naclac_lang::prelude::*;

#[event]
pub struct FooEvent {
    pub bad: Pubkey,
}

fn main() {}