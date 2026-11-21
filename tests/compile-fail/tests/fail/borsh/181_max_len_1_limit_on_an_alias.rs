#![allow(unexpected_cfgs)]
// A 1-limit `#[max_len]` on an alias of a type with fewer levels is rejected.
use naclac_lang::prelude::*;

declare_id!("11111111111111111111111111111111");

type Count = u64;

#[account]
pub struct Counter {
    #[max_len(4)]
    pub value: Count,
}

fn main() {}
