#![allow(unexpected_cfgs)]
// A handler takes exactly one `Context`.
use naclac_lang::prelude::*;

declare_id!("11111111111111111111111111111111");

#[derive(Accounts)]
pub struct Plain {}

#[program]
pub mod prog {
    pub fn set(_a: Context<Plain>, _b: Context<Plain>) -> Result {
        Ok(())
    }
}

fn main() {}
