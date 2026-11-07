#![allow(unexpected_cfgs)]
// Site: derive/src/accounts/field/init_cpi — an `init` field with no
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

#[account]
pub struct FooData {
    pub value: u64,
}

fn main() {}
