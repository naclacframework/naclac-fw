#![allow(unexpected_cfgs)]
// Site: macros/src/account.rs:29 — `#[account]` field typed
// `Pubkey` (deprecated in favor of `Address`).
use naclac_lang::prelude::*;

#[account]
pub struct Foo {
    pub bad: Pubkey,
}

fn main() {}