#![allow(unexpected_cfgs)]
// Boxing the fields is valid.
use naclac_lang::prelude::*;

declare_id!("11111111111111111111111111111111");

#[derive(Accounts)]
pub struct Foo {
    pub a: Box<Account<Medium>>,
    pub b: Box<Account<Medium>>,
    pub c: Box<Account<Medium>>,
    pub d: Box<Account<Medium>>,
    pub e: Box<Account<Medium>>,
    pub f: Box<Account<Medium>>,
    pub g: Box<Account<Medium>>,
    pub h: Box<Account<Medium>>,
    pub i: Box<Account<Medium>>,
}

#[account]
pub struct Medium {
    pub data: [u8; 200],
}

fn main() {}
