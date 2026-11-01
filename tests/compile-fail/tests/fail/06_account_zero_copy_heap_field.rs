#![allow(unexpected_cfgs)]
// Site: macros/src/account.rs:94 — a heap-allocated field
// (`Vec`/`String`) inside a zero-copy `#[account]`. Persisted zero-copy
// account data is round-tripped via raw byte-casting across separate
// transactions; a heap pointer written in one transaction is meaningless
// (or attacker-controlled) when read back in a later one.
use naclac_lang::prelude::*;

#[account]
pub struct Foo {
    pub bad: Vec<u8>,
}

fn main() {}