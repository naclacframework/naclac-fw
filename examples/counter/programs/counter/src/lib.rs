#![no_std]
use naclac_lang::prelude::*;

declare_id!("7eya8eU4wC8vfVSfa8gq68o5upVrREJwa4mKzT8uHEEs");

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
