#![allow(unexpected_cfgs)]
// Site: naclac-macros/src/instruction/security.rs — a bare `bump` with no
// `seeds = [...]` on the same field. Without `seeds`, there is no PDA to
// verify the bump against; previously this was a silent no-op (the entire
// PDA-validation block, gated on `pda_seed.is_some()`, never ran).
use naclac_lang::prelude::*;

#[derive(Accounts)]
pub struct Foo {
    #[account(bump)]
    pub account: Account<FooData>,
}

#[component]
pub struct FooData {
    pub value: u64,
}

fn main() {}
