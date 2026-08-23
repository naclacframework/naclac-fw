#![allow(unexpected_cfgs)]
// Site: naclac-macros/src/instruction/init_cpi.rs — an `init` field with no
// `system_program: Program<System>` anywhere in the struct. Previously a
// raw proc-macro panic; now a spanned `syn::Error`.
use naclac_lang::prelude::*;

#[derive(Accounts)]
pub struct Foo {
    #[account(mut)]
    pub payer: Signer,

    #[account(init, payer = payer, seeds = [b"foo"], bump = 255)]
    pub account: Account<FooData>,
}

#[component]
pub struct FooData {
    pub value: u64,
}

fn main() {}
