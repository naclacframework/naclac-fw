#![allow(unexpected_cfgs)]
// An AccountInfo with seeds supplies its bump.
use naclac_lang::prelude::*;

declare_id!("11111111111111111111111111111111");

#[derive(Accounts)]
pub struct Foo {
    pub authority: Signer,
    /// SAFETY: only its address is checked against the seeds; never read.
    #[account(seeds = [b"foo", authority.address().as_ref()], bump = 254)]
    pub account: AccountInfo,
}

fn main() {}
