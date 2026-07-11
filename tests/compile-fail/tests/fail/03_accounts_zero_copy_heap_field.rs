#![allow(unexpected_cfgs)]
// Site: naclac-macros/src/accounts.rs:270 — a bare heap type (`Vec`/`String`)
// used as an Accounts-struct field wrapper type in zero-copy mode (this
// crate has no `borsh` feature enabled, so zero-copy is the active mode).
use naclac_lang::prelude::*;

#[derive(Accounts)]
pub struct Foo {
    pub bad: Vec<u8>,
}

fn main() {}