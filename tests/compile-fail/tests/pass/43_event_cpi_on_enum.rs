#![allow(unexpected_cfgs)]
// `#[event_cpi]` goes directly above `#[derive(Accounts)]` on a struct with named fields and takes no arguments.
use naclac_lang::prelude::*;

declare_id!("11111111111111111111111111111111");

#[event_cpi]
#[derive(Accounts)]
pub struct Foo {}

fn main() {}
