#![allow(unexpected_cfgs)]
// One bump field is valid.
use naclac_lang::prelude::*;

declare_id!("11111111111111111111111111111111");

#[account]
#[seeds(b"state", bump = first)]
pub struct State {
    pub first: u8,
}

fn main() {}
