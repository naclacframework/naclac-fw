#![allow(unexpected_cfgs)]
// Site: macros/src/instruction/security.rs:701 — a relation `field = <ident>`
// (here `maker = does_not_exist`) where `<ident>` isn't an actual field
// declared in the same Accounts struct.
use naclac_lang::prelude::*;

#[derive(Accounts)]
pub struct Foo {
    #[account(maker = does_not_exist)]
    pub account: Account<FooData>,
}

#[component]
pub struct FooData {
    pub value: u64,
}

fn main() {}
