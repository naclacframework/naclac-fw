#![allow(unexpected_cfgs)]
// Instruction arguments are plain names.
use naclac_lang::prelude::*;

declare_id!("11111111111111111111111111111111");

#[derive(Accounts)]
pub struct Plain {}

#[program]
pub mod prog {
    pub fn set(_ctx: Context<Plain>, (_a, _b): (u8, u8)) -> Result {
        Ok(())
    }
}

fn main() {}
