#![allow(unexpected_cfgs)]
// Site: naclac-macros/src/instruction/security.rs:701 — `has_one = <ident>`
// where `<ident>` isn't an actual field declared in the same Accounts
// struct.
use naclac_lang::prelude::*;

#[derive(Accounts)]
pub struct Foo {
    #[account(has_one = does_not_exist)]
    pub account: Account<FooData>,
}

#[component]
pub struct FooData {
    pub value: u64,
}

fn main() {}