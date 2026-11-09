#![allow(unexpected_cfgs)]
// Site: derive/src/accounts/field/parser/validate.rs — `dup` needs a
// `/// SAFETY:` doc comment.
use naclac_lang::prelude::*;

#[derive(Accounts)]
pub struct Foo {
    pub a: Signer,
    #[account(dup)]
    pub b: Signer,
}

fn main() {}
