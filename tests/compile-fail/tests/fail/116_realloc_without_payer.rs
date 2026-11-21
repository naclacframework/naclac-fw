#![allow(unexpected_cfgs)]
// `realloc` needs an explicit `realloc::payer`.
use naclac_lang::prelude::*;

declare_id!("11111111111111111111111111111111");

#[derive(Accounts)]
#[instruction(new_space: u64)]
pub struct Foo {
    #[account(mut)]
    pub payer: Signer,

    #[account(mut, realloc = new_space as usize)]
    pub account: Account<FooData>,

    pub system_program: Program<System>,
}

#[account]
pub struct FooData {
    pub value: u64,
}

fn main() {}
