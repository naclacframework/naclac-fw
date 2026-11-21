#![allow(unexpected_cfgs)]
// A `#[max_len]` needs a `Vec` or `String` to limit.
use naclac_lang::prelude::*;

declare_id!("11111111111111111111111111111111");

#[account]
pub struct Counter {
    #[max_len(4)]
    pub value: u64,
}

fn main() {}
