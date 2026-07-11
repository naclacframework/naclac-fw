#![allow(unexpected_cfgs)]
// Valid counterpart to tests/fail/16_accountinfo_missing_safety_comment.rs
// — a `/// SAFETY: ...` doc comment directly above the field satisfies the
// check.
use naclac_lang::prelude::*;

#[derive(Accounts)]
pub struct Foo {
    /// SAFETY: only read to compare against a stored value elsewhere; never
    /// deserialized or written.
    pub unchecked: AccountInfo,
}

fn main() {}