#![allow(unexpected_cfgs)]
// An Accounts field is an account type, not a bare address.
use naclac_lang::prelude::*;

declare_id!("11111111111111111111111111111111");

#[derive(Accounts)]
pub struct Foo {
    pub owner: Signer,
}

fn main() {}
