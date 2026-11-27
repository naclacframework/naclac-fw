#![allow(unexpected_cfgs)]
// Valid counterpart to tests/fail/206_explicit_bump_on_constant_seeds.rs: a bare `bump` on constant seeds.
use naclac_lang::prelude::*;

declare_id!("11111111111111111111111111111111");

#[derive(Accounts)]
pub struct Foo {
    /// SAFETY: only its address is checked against the seeds; never read.
    #[account(seeds = [b"foo"], bump)]
    pub account: UncheckedAccount,
}

fn main() {}
