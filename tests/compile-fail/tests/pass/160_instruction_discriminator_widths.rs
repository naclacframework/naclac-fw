#![allow(unexpected_cfgs)]
// Equal widths are valid.
use naclac_lang::prelude::*;

declare_id!("11111111111111111111111111111111");

#[derive(Accounts)]
pub struct Plain {}

#[program]
pub mod prog {
    #[discriminator = [1, 2]]
    pub fn first(_ctx: Context<Plain>) -> Result {
        Ok(())
    }
    #[discriminator = [3, 4]]
    pub fn second(_ctx: Context<Plain>) -> Result {
        Ok(())
    }
}

fn main() {}
