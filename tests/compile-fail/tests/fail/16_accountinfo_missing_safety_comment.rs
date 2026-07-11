#![allow(unexpected_cfgs)]
// Site: naclac-macros/src/instruction/parser.rs:333 — a bare `AccountInfo`
// field receives no automatic validation and requires a `/// SAFETY: ...`
// doc comment directly above it justifying why that's acceptable.
use naclac_lang::prelude::*;

#[derive(Accounts)]
pub struct Foo {
    pub unchecked: AccountInfo,
}

fn main() {}