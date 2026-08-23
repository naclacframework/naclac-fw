#![no_std]
use naclac_lang::prelude::*;

declare_id!("4rY4SAimzM1LzDaG8zdv2XV3tdKQbd12tck8Jzv7sZc8");

pub mod components;
pub mod instructions;
pub mod systems;
pub mod events;
pub mod errors;
pub mod constants;

use instructions::*;

#[program]
pub mod counter {
    pub fn initialize(ctx: Context<Initialize>) -> Result {
        initialize::initialize(ctx)
    }

    pub fn increment(ctx: Context<Increment>) -> Result {
        increment::increment(ctx)
    }
}
