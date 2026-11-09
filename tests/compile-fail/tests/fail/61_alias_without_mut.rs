#![allow(unexpected_cfgs)]
// Site: derive/src/accounts/field/parser/mod.rs — `unsafe(alias)` is for `mut`
// fields; a read-only field that may repeat an account uses `dup`.
use naclac_lang::prelude::*;

#[derive(Accounts)]
pub struct Foo {
    pub a: Signer,
    #[account(unsafe(alias))]
    pub b: Signer,
}

fn main() {}
