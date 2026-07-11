#![allow(unexpected_cfgs)]
// Site: naclac-macros/src/instruction/parser.rs:175 — a relation
// constraint's KEY must be a simple identifier. `foo::bar` isn't one of the
// recognized two-segment reserved forms (`token::*`, `mint::*`,
// `seeds::program`), so it falls through to the generic relation-constraint
// path, where `path.get_ident()` fails on a multi-segment path.
use naclac_lang::prelude::*;

#[derive(Accounts)]
pub struct Foo {
    #[account(foo::bar = owner)]
    pub account: Signer,
    pub owner: Signer,
}

fn main() {}