#![allow(unexpected_cfgs)]
// A matching number of limits on an alias is valid.
use naclac_lang::prelude::*;

declare_id!("11111111111111111111111111111111");

type Values = Vec<u64>;

#[account]
pub struct Counter {
    #[max_len(4)]
    pub values: Values,
}

fn main() {}
