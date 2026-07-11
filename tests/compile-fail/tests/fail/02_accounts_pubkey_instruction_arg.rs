#![allow(unexpected_cfgs)]
// Site: naclac-macros/src/accounts.rs:93 — `#[instruction(...)]` arg typed
// `Pubkey` (deprecated in favor of `Address`).
use naclac_lang::prelude::*;
use naclac_lang::solana_program::pubkey::Pubkey;

#[derive(Accounts)]
#[instruction(bad: Pubkey)]
pub struct Foo {}

fn main() {}