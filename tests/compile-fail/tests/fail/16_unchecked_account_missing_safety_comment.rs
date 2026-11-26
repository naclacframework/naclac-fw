#![allow(unexpected_cfgs)]
// Site: derive/src/accounts/field/parser/validate.rs — a bare `UncheckedAccount`
// field receives no automatic validation and requires a `/// SAFETY: ...`
// doc comment directly above it justifying why that's acceptable.
use naclac_lang::prelude::*;

#[derive(Accounts)]
pub struct Foo {
    pub unchecked: UncheckedAccount,
}

fn main() {}