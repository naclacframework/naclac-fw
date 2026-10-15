#![allow(unexpected_cfgs)]
// Site: naclac-macros/src/accounts.rs:93 — `#[instruction(...)]` arg typed
// `Pubkey` (deprecated in favor of `Address`).
use naclac_lang::prelude::*;

#[derive(Accounts)]
#[instruction(bad: Pubkey)]
pub struct Foo {}

fn main() {}