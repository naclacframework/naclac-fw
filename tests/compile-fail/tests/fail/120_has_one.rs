#![allow(unexpected_cfgs)]
// `has_one` is not supported; the relation names the field.
use naclac_lang::prelude::*;

declare_id!("11111111111111111111111111111111");

#[derive(Accounts)]
pub struct Foo {
    #[account(has_one = maker)]
    pub account: Account<FooData>,
    pub maker: Signer,
}

#[account]
pub struct FooData {
    pub maker: Address,
}

fn main() {}
