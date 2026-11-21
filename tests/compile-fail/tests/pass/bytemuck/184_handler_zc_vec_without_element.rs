#![allow(unexpected_cfgs)]
// A `ZcVec<u8>` argument is valid.
use naclac_lang::prelude::*;

declare_id!("11111111111111111111111111111111");

#[derive(Accounts)]
pub struct Plain {}

#[program]
pub mod prog {
    pub fn set(_ctx: Context<Plain>, _data: ZcVec<u8>) -> Result {
        Ok(())
    }
}

fn main() {}
