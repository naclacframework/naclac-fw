#![allow(unexpected_cfgs)]
// Valid counterpart to tests/fail/58_dup_with_mut.rs and
// tests/fail/59_dup_without_safety_comment.rs — a documented read-only field,
// including an optional one, may be marked `dup`.
use naclac_lang::prelude::*;

declare_id!("11111111111111111111111111111111");

#[derive(Accounts)]
pub struct Foo {
    pub a: Signer,
    /// SAFETY: test fixture.
    #[account(dup)]
    pub b: Signer,
    /// SAFETY: test fixture.
    #[account(dup)]
    pub c: Option<AccountInfo>,
}

fn main() {}
