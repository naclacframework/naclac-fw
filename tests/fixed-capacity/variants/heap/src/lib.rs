#![no_std]
use naclac_lang::prelude::*;

declare_id!("11111111111111111111111111111111");

pub mod instructions;
pub mod state;

#[cfg(test)]
mod tests;

use instructions::*;

#[program]
pub mod capacity_borsh {
    pub fn init_roster(ctx: Context<InitRoster>) -> Result {
        instructions::init_roster(ctx)
    }

    pub fn fill_roster(ctx: Context<Mutate>, count: u8) -> Result {
        instructions::fill_roster(ctx, count)
    }

    pub fn set_label(ctx: Context<Mutate>, label: String) -> Result {
        instructions::set_label(ctx, label)
    }
}
