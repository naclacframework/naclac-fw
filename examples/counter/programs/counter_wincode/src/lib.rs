use {
    instructions::*,
    naclac_lang::prelude::*,
};

declare_id!("DDmzYuMpywidtCzyfR8PVjEvR4jeNKGpBjGHE419kKPZ");

pub mod components;
pub mod instructions;
pub mod systems;
pub mod events;
pub mod errors;
pub mod constants;

#[program]
pub mod counter_wincode {
    pub fn initialize(ctx: Context<Initialize>) -> Result {
        initialize::initialize(ctx)
    }

    pub fn increment(ctx: Context<Increment>) -> Result {
        increment::increment(ctx)
    }

    pub fn decrement(ctx: Context<Decrement>) -> Result {
        decrement::decrement(ctx)
    }
}
