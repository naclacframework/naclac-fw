#![allow(unexpected_cfgs)]
// The explicit `unsafe(alias)` spelling is valid.
use naclac_lang::prelude::*;

#[derive(Accounts)]
pub struct Foo {
    #[account(mut, unsafe(alias))]
    pub a: Signer,
    #[account(mut, unsafe(alias))]
    pub b: Signer,
}

fn main() {}
