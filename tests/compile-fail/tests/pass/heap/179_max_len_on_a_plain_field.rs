#![allow(unexpected_cfgs)]
// A `#[max_len]` on a `Vec` is valid.
use naclac_lang::prelude::*;

declare_id!("11111111111111111111111111111111");

#[account]
pub struct Counter {
    #[max_len(4)]
    pub values: Vec<u64>,
}

fn main() {}
