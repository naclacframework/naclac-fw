#![allow(unexpected_cfgs)]
// Site: naclac-macros/src/accounts.rs:41 — Accounts-struct field typed
// `Pubkey` (deprecated in favor of `Address`).
use naclac_lang::prelude::*;
use naclac_lang::solana_program::pubkey::Pubkey;

#[derive(Accounts)]
pub struct Foo {
    pub bad: Pubkey,
}

fn main() {}