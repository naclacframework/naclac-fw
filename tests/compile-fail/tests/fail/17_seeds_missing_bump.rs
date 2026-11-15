#![allow(unexpected_cfgs)]
// A `seeds = [...]` constraint with no `bump` on an existing account whose
// type does not prove the deriving program created it (`AccountInfo`).
// On-chain bump discovery skips the curve check, so it is only allowed for
// `Account<T>`/`InterfaceAccount<T>`; any other type must supply its bump.
//
// The second seed names another field, so the PDA is derived at runtime and
// the field needs a bump source.
use naclac_lang::prelude::*;

#[derive(Accounts)]
pub struct Foo {
    pub authority: Signer,

    /// SAFETY: only its address is checked against the seeds; never read.
    #[account(seeds = [b"foo", authority.address().as_ref()])]
    pub account: AccountInfo,
}

fn main() {}
