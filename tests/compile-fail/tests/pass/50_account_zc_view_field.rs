#![allow(unexpected_cfgs)]
// An account stores plain fixed-size fields.
use naclac_lang::prelude::*;

#[account]
pub struct Profile {
    pub owner: Address,
    pub id: u64,
}

fn main() {}
