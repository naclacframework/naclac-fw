#![allow(unexpected_cfgs)]
// A `#[seeds]` names one bump field.
use naclac_lang::prelude::*;

declare_id!("11111111111111111111111111111111");

#[account]
#[seeds(b"state", bump = first, bump = second)]
pub struct State {
    pub first: u8,
    pub second: u8,
}

fn main() {}
