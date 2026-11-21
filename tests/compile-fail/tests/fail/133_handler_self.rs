#![allow(unexpected_cfgs)]
// An instruction handler is a free function, not a method.
use naclac_lang::prelude::*;

declare_id!("11111111111111111111111111111111");

#[derive(Accounts)]
pub struct Plain {}

#[program]
pub mod prog {
    pub fn set(&self) -> Result {
        Ok(())
    }
}

fn main() {}
