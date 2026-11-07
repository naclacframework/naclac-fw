#![allow(unexpected_cfgs)]
// Site: derive/src/accounts/mod.rs — `#[instruction(...)]` arg typed
// `Pubkey` (deprecated in favor of `Address`).
use naclac_lang::prelude::*;

#[derive(Accounts)]
#[instruction(bad: Pubkey)]
pub struct Foo {}

fn main() {}