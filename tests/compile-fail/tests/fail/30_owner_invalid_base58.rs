#![allow(unexpected_cfgs)]
// Site: naclac-macros/src/instruction/security.rs — `owner = "..."` with a
// string that doesn't decode as valid base58. Previously a raw proc-macro
// panic (`.expect()`); now a spanned `syn::Error`.
use naclac_lang::prelude::*;

#[derive(Accounts)]
pub struct Foo {
    /// SAFETY: fully validated via `owner =` below — irrelevant here, this
    /// fixture never reaches that check.
    #[account(owner = "not-valid-base58!!!")]
    pub account: AccountInfo,
}

fn main() {}
