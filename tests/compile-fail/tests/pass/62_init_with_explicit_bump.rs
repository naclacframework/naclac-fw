#![allow(unexpected_cfgs)]
// An init field takes a bare `bump`; the framework finds it.
use naclac_lang::prelude::*;

declare_id!("11111111111111111111111111111111");

#[derive(Accounts)]
#[instruction(id: u64)]
pub struct Foo {
    #[account(mut)]
    pub payer: Signer,

    #[account(init, payer = payer, seeds = [b"entry", &id.to_le_bytes()], bump)]
    pub entry: Account<Entry>,

    pub system_program: Program<System>,
}

#[account]
pub struct Entry {
    pub value: u64,
}

fn main() {}
