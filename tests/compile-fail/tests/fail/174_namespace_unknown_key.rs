#![allow(unexpected_cfgs)]
// A `mint::` key is one of the namespace's own keys.
use naclac_lang::prelude::*;

#[derive(Accounts)]
pub struct Foo {
    /// SAFETY: only used as the mint authority; never read or deserialized.
    pub owner: AccountInfo,

    #[account(mint::bogus = owner)]
    pub mint: Account<Mint>,
}

fn main() {}
