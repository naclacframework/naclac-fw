#![allow(unexpected_cfgs)]
// A seed of a `Seeds` type is at most 32 bytes.
use naclac_lang::prelude::*;

declare_id!("11111111111111111111111111111111");

#[derive(Seeds)]
#[seeds(b"state", key: [u8; 33])]
pub struct StateSeeds;

fn main() {}
