#![allow(unexpected_cfgs)]
// A `seeds = [...]` constraint with no `bump` on an existing account whose
// type does not prove the deriving program created it (`AccountInfo`).
// On-chain bump discovery skips the curve check, so it is only allowed for
// `Account<T>`/`InterfaceAccount<T>`; any other type must supply its bump.
//
// This fixture has no `Naclac.toml`, so no program ID resolves at compile
// time and every `seeds = [...]` here takes the dynamic-PDA branch.
use naclac_lang::prelude::*;

#[derive(Accounts)]
pub struct Foo {
    /// SAFETY: only its address is checked against the seeds; never read.
    #[account(seeds = [b"foo"])]
    pub account: AccountInfo,
}

fn main() {}
