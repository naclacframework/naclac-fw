#![allow(unexpected_cfgs)]
// An account with named fields is valid.
use naclac_lang::prelude::*;

declare_id!("11111111111111111111111111111111");

#[account]
#[seeds(b"state", bump = bump)]
pub struct State {
    pub bump: u8,
}

fn main() {}
