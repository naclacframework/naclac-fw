#![allow(unexpected_cfgs)]
// Site: naclac-macros/src/component.rs:29 — `#[component]` field typed
// `Pubkey` (deprecated in favor of `Address`).
use naclac_lang::prelude::*;

#[component]
pub struct Foo {
    pub bad: Pubkey,
}

fn main() {}