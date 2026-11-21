#![allow(unexpected_cfgs)]
// A short prefix is valid.
use naclac_lang::prelude::*;

declare_id!("11111111111111111111111111111111");

#[derive(Seeds)]
#[seeds(b"state", key: Address)]
pub struct StateSeeds;

fn main() {}
