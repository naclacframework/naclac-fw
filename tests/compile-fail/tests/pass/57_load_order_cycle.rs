#![allow(unexpected_cfgs)]
// Seeds that depend on each other in one direction load in dependency order.
use naclac_lang::prelude::*;

declare_id!("11111111111111111111111111111111");

#[derive(Accounts)]
pub struct Chain {
    #[account(seeds = [b"a", second.address().as_ref()], bump = 1)]
    pub first: Account<Thing>,

    #[account(seeds = [b"b"], bump = 1)]
    pub second: Account<Thing>,
}

#[account]
pub struct Thing {
    pub value: u64,
}

fn main() {}
