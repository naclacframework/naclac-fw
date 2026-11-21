#![allow(unexpected_cfgs)]
// `init` and `init_if_needed` are two creation modes; a field takes one.
use naclac_lang::prelude::*;

declare_id!("11111111111111111111111111111111");

#[derive(Accounts)]
pub struct Foo {
    #[account(mut)]
    pub payer: Signer,

    #[account(init, init_if_needed, payer = payer, seeds = [b"foo", payer.address().as_ref()], bump)]
    pub account: Account<FooData>,

    pub system_program: Program<System>,
}

#[account]
pub struct FooData {
    pub value: u64,
}

fn main() {}
