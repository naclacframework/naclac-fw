#![allow(unexpected_cfgs)]
// An account field over 300 bytes must be boxed to stay within the SBF stack frame.
use naclac_lang::prelude::*;

declare_id!("11111111111111111111111111111111");

#[derive(Accounts)]
pub struct Foo {
    pub big: Account<Big>,
}

#[account]
pub struct Big {
    pub data: [u8; 400],
}

fn main() {}
