#![allow(unexpected_cfgs)]
// A `Vec` handler argument names its element type.
use naclac_lang::prelude::*;

declare_id!("11111111111111111111111111111111");

#[derive(Accounts)]
pub struct Plain {}

#[program]
pub mod prog {
    pub fn set(_ctx: Context<Plain>, _data: Vec) -> Result {
        Ok(())
    }
}

fn main() {}
