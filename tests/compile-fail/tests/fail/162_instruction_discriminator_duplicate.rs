#![allow(unexpected_cfgs)]
// Two instructions cannot share a discriminator.
use naclac_lang::prelude::*;

declare_id!("11111111111111111111111111111111");

#[derive(Accounts)]
pub struct Plain {}

#[program]
pub mod prog {
    #[discriminator = 1]
    pub fn first(_ctx: Context<Plain>) -> Result {
        Ok(())
    }
    #[discriminator = 1]
    pub fn second(_ctx: Context<Plain>) -> Result {
        Ok(())
    }
}

fn main() {}
