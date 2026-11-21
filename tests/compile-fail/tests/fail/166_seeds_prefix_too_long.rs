#![allow(unexpected_cfgs)]
// The prefix of a `#[seeds]` is at most 32 bytes.
use naclac_lang::prelude::*;

declare_id!("11111111111111111111111111111111");

#[derive(Seeds)]
#[seeds(b"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", key: Address)]
pub struct StateSeeds;

fn main() {}
