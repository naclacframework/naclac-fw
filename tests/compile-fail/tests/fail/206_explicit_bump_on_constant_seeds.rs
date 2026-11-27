#![allow(unexpected_cfgs)]
// Every seed is a constant, so the bump is found at compile time and `bump = <value>` is refused.
use naclac_lang::prelude::*;

declare_id!("11111111111111111111111111111111");

#[derive(Accounts)]
pub struct Foo {
    /// SAFETY: only its address is checked against the seeds; never read.
    #[account(seeds = [b"foo"], bump = 254)]
    pub account: UncheckedAccount,
}

fn main() {}
