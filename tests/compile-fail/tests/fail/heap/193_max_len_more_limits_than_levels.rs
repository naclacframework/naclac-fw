#![allow(unexpected_cfgs)]
// A `String` has one level, so a second `#[max_len]` limit has nothing to apply to.
use naclac_lang::prelude::*;

declare_id!("11111111111111111111111111111111");

#[account]
pub struct Profile {
    #[max_len(4, 8)]
    pub name: String,
}

fn main() {}
