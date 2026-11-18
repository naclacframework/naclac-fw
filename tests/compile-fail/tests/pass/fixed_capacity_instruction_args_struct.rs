#![allow(unexpected_cfgs)]

use naclac_lang::prelude::*;

declare_id!("11111111111111111111111111111111");

#[instruction_args]
pub struct Roster {
    pub scores: FixedVec<u64, 4>,
    pub name: FixedString<8>,
    pub level: u8,
}

#[derive(Accounts)]
pub struct Plain {}

#[program]
pub mod prog {
    pub fn set(_ctx: Context<Plain>, _roster: Roster) -> Result {
        Ok(())
    }
}

fn main() {}
