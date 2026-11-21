#![allow(unexpected_cfgs)]
// One limit for the one level of a `String` is valid.
use naclac_lang::prelude::*;

declare_id!("11111111111111111111111111111111");

#[account]
pub struct Profile {
    #[max_len(4)]
    pub name: String,
}

fn main() {}
