#![allow(unexpected_cfgs)]
// The override is written `#[discriminator = <value>]`.
use naclac_lang::prelude::*;

declare_id!("11111111111111111111111111111111");

#[derive(Accounts)]
pub struct Plain {}

#[program]
pub mod prog {
    #[discriminator(7)]
    pub fn set(_ctx: Context<Plain>) -> Result {
        Ok(())
    }
}

fn main() {}
