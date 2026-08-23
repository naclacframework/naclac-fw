#![allow(unexpected_cfgs)]
// Site: naclac-macros/src/instruction/security.rs — `owner = "..."` that
// decodes as valid base58 but not to exactly 32 bytes. Previously a raw
// proc-macro panic (`assert_eq!`); now a spanned `syn::Error`.
use naclac_lang::prelude::*;

#[derive(Accounts)]
pub struct Foo {
    /// SAFETY: fully validated via `owner =` below — irrelevant here, this
    /// fixture never reaches that check.
    // "11111111" decodes to a handful of zero bytes — valid base58, wrong length.
    #[account(owner = "11111111")]
    pub account: AccountInfo,
}

fn main() {}
