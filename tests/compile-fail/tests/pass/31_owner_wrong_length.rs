#![allow(unexpected_cfgs)]
// An `owner` decodes to 32 bytes.
use naclac_lang::prelude::*;

#[derive(Accounts)]
pub struct Foo {
    /// SAFETY: only the owner is checked; the account is never read.
    #[account(owner = "11111111111111111111111111111111")]
    pub account: AccountInfo,
}

fn main() {}
