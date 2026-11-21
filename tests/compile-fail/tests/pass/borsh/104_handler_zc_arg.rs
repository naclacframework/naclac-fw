#![allow(unexpected_cfgs)]
// A handler argument of type `Vec<u8>` is valid under Borsh and Wincode.
use naclac_lang::prelude::*;

declare_id!("11111111111111111111111111111111");

#[derive(Accounts)]
pub struct Plain {}

#[program]
pub mod prog {
    pub fn set(_ctx: Context<Plain>, _data: Vec<u8>) -> Result {
        Ok(())
    }
}

fn main() {}
