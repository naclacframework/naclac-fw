#![allow(unexpected_cfgs)]
// A relation constraint is written with a plain field name.
use naclac_lang::prelude::*;

declare_id!("11111111111111111111111111111111");

#[derive(Accounts)]
pub struct Foo {
    #[account(::maker = maker)]
    pub account: Account<FooData>,
    pub maker: Signer,
}

#[account]
pub struct FooData {
    pub maker: Address,
}

fn main() {}
