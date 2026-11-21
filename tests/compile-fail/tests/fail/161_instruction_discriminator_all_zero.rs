#![allow(unexpected_cfgs)]
// A multi-byte instruction discriminator is never all zero.
use naclac_lang::prelude::*;

declare_id!("11111111111111111111111111111111");

#[derive(Accounts)]
pub struct Plain {}

#[program]
pub mod prog {
    #[discriminator = [0, 0]]
    pub fn set(_ctx: Context<Plain>) -> Result {
        Ok(())
    }
}

fn main() {}
