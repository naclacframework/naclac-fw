#![allow(unexpected_cfgs)]
// A `#[seeds]` that reads a field needs an account with named fields.
use naclac_lang::prelude::*;

declare_id!("11111111111111111111111111111111");

#[account]
#[seeds(b"state", bump = 0)]
pub struct State(u8);

fn main() {}
