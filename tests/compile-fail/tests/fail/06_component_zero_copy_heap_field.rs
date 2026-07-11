#![allow(unexpected_cfgs)]
// Site: naclac-macros/src/component.rs:94 — a heap-allocated field
// (`Vec`/`String`) inside a zero-copy `#[component]`. Persisted zero-copy
// account data is round-tripped via raw byte-casting across separate
// transactions; a heap pointer written in one transaction is meaningless
// (or attacker-controlled) when read back in a later one.
use naclac_lang::prelude::*;

#[component]
pub struct Foo {
    pub bad: Vec<u8>,
}

fn main() {}