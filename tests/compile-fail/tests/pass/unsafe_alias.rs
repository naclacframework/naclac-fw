#![allow(unexpected_cfgs)]
// Valid counterpart to tests/fail/14_bare_alias.rs — `unsafe(alias)` is the
// correct, explicit way to opt out of duplicate-mutable-account protection.
use naclac_lang::prelude::*;

#[derive(Accounts)]
pub struct Foo {
    #[account(mut, unsafe(alias))]
    pub a: Signer,
    #[account(mut, unsafe(alias))]
    pub b: Signer,
}

fn main() {}