#![allow(unexpected_cfgs)]
// `extra_accounts` belongs to an `init_if_needed` associated token account.
use naclac_lang::prelude::*;

declare_id!("11111111111111111111111111111111");

#[derive(Accounts)]
pub struct Foo {
    #[account(mut)]
    pub payer: Signer,

    #[account(init, payer = payer, seeds = [b"foo", payer.address().as_ref()], bump, extra_accounts = [payer])]
    pub account: Account<FooData>,

    pub system_program: Program<System>,
}

#[account]
pub struct FooData {
    pub value: u64,
}

fn main() {}
