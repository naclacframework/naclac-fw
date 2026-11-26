#![allow(unexpected_cfgs)]
// An `owner` is a canonical base58 address.
use naclac_lang::prelude::*;

#[derive(Accounts)]
pub struct Foo {
    /// SAFETY: only the owner is checked; the account is never read.
    #[account(owner = "11111111111111111111111111111111")]
    pub account: UncheckedAccount,
}

fn main() {}
