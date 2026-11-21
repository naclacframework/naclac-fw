#![allow(unexpected_cfgs)]
// A boxed account is valid.
use naclac_lang::prelude::*;

declare_id!("11111111111111111111111111111111");

#[derive(Accounts)]
pub struct Foo {
    pub big: Box<Account<Big>>,
}

#[account]
pub struct Big {
    pub data: [u8; 400],
}

fn main() {}
