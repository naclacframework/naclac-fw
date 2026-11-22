#![allow(unexpected_cfgs)]
// A close destination names a field of the same Accounts struct.
use naclac_lang::prelude::*;

declare_id!("11111111111111111111111111111111");

#[derive(Accounts)]
pub struct Foo {
    #[account(mut, close = receiver)]
    pub account: Account<FooData>,

    #[account(mut)]
    pub receiver: Signer,
}

#[account]
pub struct FooData {
    pub value: u64,
}

fn main() {}
