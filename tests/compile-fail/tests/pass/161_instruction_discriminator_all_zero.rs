#![allow(unexpected_cfgs)]
// A discriminator with a non-zero byte is valid.
use naclac_lang::prelude::*;

declare_id!("11111111111111111111111111111111");

#[derive(Accounts)]
pub struct Plain {}

#[program]
pub mod prog {
    #[discriminator = [0, 1]]
    pub fn set(_ctx: Context<Plain>) -> Result {
        Ok(())
    }
}

fn main() {}
