#![allow(unexpected_cfgs)]
// Site: derive/src/account/mod.rs — `#[account]` field typed
// `Pubkey` (deprecated in favor of `Address`).
use naclac_lang::prelude::*;

#[account]
pub struct Foo {
    pub bad: Pubkey,
}

fn main() {}