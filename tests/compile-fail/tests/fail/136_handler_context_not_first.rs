#![allow(unexpected_cfgs)]
// The `Context` argument comes first.
use naclac_lang::prelude::*;

declare_id!("11111111111111111111111111111111");

#[derive(Accounts)]
pub struct Plain {}

#[program]
pub mod prog {
    pub fn set(_amount: u64, _ctx: Context<Plain>) -> Result {
        Ok(())
    }
}

fn main() {}
