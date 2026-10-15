#![allow(unexpected_cfgs)]
// Site: macros/src/accounts.rs:41 — Accounts-struct field typed
// `Pubkey` (deprecated in favor of `Address`).
use naclac_lang::prelude::*;
use naclac_lang::solana_address::Address as Pubkey;

#[derive(Accounts)]
pub struct Foo {
    pub bad: Pubkey,
}

fn main() {}