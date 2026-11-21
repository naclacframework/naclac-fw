#![allow(unexpected_cfgs)]
// A 32-byte seed is valid.
use naclac_lang::prelude::*;

declare_id!("11111111111111111111111111111111");

#[derive(Seeds)]
#[seeds(b"state", key: [u8; 32])]
pub struct StateSeeds;

fn main() {}
