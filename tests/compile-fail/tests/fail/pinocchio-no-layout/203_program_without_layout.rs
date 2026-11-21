#![allow(unexpected_cfgs)]
// A program with no account-data layout feature is rejected.
use naclac_lang::prelude::*;

declare_id!("11111111111111111111111111111111");

#[derive(Accounts)]
pub struct Plain {}

#[program]
pub mod prog {
    pub fn set(_ctx: Context<Plain>) -> Result {
        Ok(())
    }
}

fn main() {}
