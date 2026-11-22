#![allow(unexpected_cfgs)]
// An instruction argument is typed `Address`.
use naclac_lang::prelude::*;

declare_id!("11111111111111111111111111111111");

#[derive(Accounts)]
#[instruction(good: Address)]
pub struct Foo {}

fn main() {}
