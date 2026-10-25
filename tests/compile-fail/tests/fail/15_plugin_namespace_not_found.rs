#![allow(unexpected_cfgs)]
// A `namespace::key = value` constraint is resolved through the module the
// namespace names, which must export `Args`, `Constraint` and `RULES`. A
// namespace that names no such module is a compile error from rustc.
use naclac_lang::prelude::*;

#[derive(Accounts)]
pub struct Foo {
    #[account(foo::bar = owner)]
    pub account: Signer,
    pub owner: Signer,
}

fn main() {}
