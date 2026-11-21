#![allow(unexpected_cfgs)]
// A known `mint::` key is valid.
use naclac_lang::prelude::*;

#[derive(Accounts)]
pub struct Foo {
    /// SAFETY: only used as the mint authority; never read or deserialized.
    pub owner: AccountInfo,

    #[account(mint::authority = owner)]
    pub mint: Account<Mint>,
}

fn main() {}
