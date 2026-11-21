#![allow(unexpected_cfgs)]
// `#[program]` needs a module with a body.
use naclac_lang::prelude::*;

declare_id!("11111111111111111111111111111111");

#[program]
pub mod prog;

fn main() {}
