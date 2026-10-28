#![allow(unexpected_cfgs)]
// Each field's seeds name the other field's address, so neither can be loaded
// first: the derive rejects the cycle instead of picking an order.
use naclac_lang::prelude::*;

#[derive(Accounts)]
pub struct Cycle {
    #[account(seeds = [b"a", second.address().as_ref()], bump = 1)]
    pub first: Account<Thing>,
    #[account(seeds = [b"b", first.address().as_ref()], bump = 1)]
    pub second: Account<Thing>,
}

#[component]
pub struct Thing {
    pub value: u64,
}

fn main() {}
