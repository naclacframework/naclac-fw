#![allow(unexpected_cfgs)]
// Site: derive/src/accounts/mod.rs — Accounts-struct field typed
// `Pubkey` (deprecated in favor of `Address`).
use naclac_lang::prelude::*;
type Pubkey = Address;

#[derive(Accounts)]
pub struct Foo {
    pub bad: Pubkey,
}

fn main() {}