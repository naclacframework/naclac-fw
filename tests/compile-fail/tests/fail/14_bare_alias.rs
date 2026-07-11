#![allow(unexpected_cfgs)]
// Site: naclac-macros/src/instruction/parser.rs:137 — bare `alias` (no
// `unsafe(...)`) is rejected; must write `unsafe(alias)` to explicitly opt
// out of duplicate-mutable-account protection.
use naclac_lang::prelude::*;

#[derive(Accounts)]
pub struct Foo {
    #[account(mut, alias)]
    pub a: Signer,
    #[account(mut, alias)]
    pub b: Signer,
}

fn main() {}