#![allow(unexpected_cfgs)]

use naclac_lang::prelude::*;

declare_id!("11111111111111111111111111111111");

#[defined_type]
pub struct Entry {
    pub scores: FixedVec<u64, 4>,
    pub name: FixedString<8>,
}

#[derive(Accounts)]
pub struct Plain {}

#[program]
pub mod prog {
    pub fn set(_ctx: Context<Plain>, _entry: Entry) -> Result {
        Ok(())
    }
}

fn main() {}
