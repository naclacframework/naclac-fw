#![allow(unexpected_cfgs)]
// Account fields that are each small but together exceed the stack budget are rejected.
use naclac_lang::prelude::*;

declare_id!("11111111111111111111111111111111");

#[derive(Accounts)]
pub struct Foo {
    pub a: Account<Medium>,
    pub b: Account<Medium>,
    pub c: Account<Medium>,
    pub d: Account<Medium>,
    pub e: Account<Medium>,
    pub f: Account<Medium>,
    pub g: Account<Medium>,
    pub h: Account<Medium>,
    pub i: Account<Medium>,
}

#[account]
pub struct Medium {
    pub data: [u8; 200],
}

fn main() {}
