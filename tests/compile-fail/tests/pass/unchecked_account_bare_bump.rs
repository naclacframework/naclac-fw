#![allow(unexpected_cfgs)]
// An UncheckedAccount with seeds and a bare `bump` gets the canonical bump found.
use naclac_lang::prelude::*;

declare_id!("11111111111111111111111111111111");

#[derive(Accounts)]
pub struct Foo {
    pub authority: Signer,
    /// SAFETY: only its address is checked against the seeds; never read.
    #[account(seeds = [b"foo", authority.address().as_ref()], bump)]
    pub account: UncheckedAccount,
}

fn main() {}
