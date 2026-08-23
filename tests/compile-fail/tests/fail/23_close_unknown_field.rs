#![allow(unexpected_cfgs)]
// Site: naclac-macros/src/instruction/close_account.rs — `close = dest`
// where `dest` doesn't name a real field on the struct. `has_one`-style
// relations already reject an unknown target field with a clear
// `compile_error!`; `close =` previously had no equivalent check and fell
// through to rustc's own "no field `dest`" error on the generated teardown
// body instead.
use naclac_lang::prelude::*;

#[derive(Accounts)]
pub struct Foo {
    #[account(mut, close = nonexistent_field)]
    pub account: Account<FooData>,
}

#[component]
pub struct FooData {
    pub value: u64,
}

fn main() {}
