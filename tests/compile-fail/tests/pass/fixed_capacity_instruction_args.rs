#![allow(unexpected_cfgs)]

use naclac_lang::prelude::*;

declare_id!("11111111111111111111111111111111");

#[derive(Accounts)]
pub struct Plain {}

#[program]
pub mod prog {
    pub fn set(_ctx: Context<Plain>, _scores: FixedVec<u64, 4>, _name: FixedString<8>) -> Result {
        Ok(())
    }
}

fn main() {}
