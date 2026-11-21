#![allow(unexpected_cfgs)]
// A creation mode needs `payer = <account>` on the same field.
use naclac_lang::prelude::*;

declare_id!("11111111111111111111111111111111");

#[derive(Accounts)]
pub struct Foo {
    #[account(mut)]
    pub payer: Signer,

    #[account(init, seeds = [b"foo", payer.address().as_ref()], bump)]
    pub account: Account<FooData>,

    pub system_program: Program<System>,
}

#[account]
pub struct FooData {
    pub value: u64,
}

fn main() {}
