use {
    instructions::*,
    naclac_lang::prelude::*,
};

declare_id!("3ssHbWJWpTnjkVjycjTecdnfSN4STz3iENDdLw8o6gdM");

pub mod components;
pub mod instructions;
pub mod systems;
pub mod events;
pub mod errors;
pub mod constants;

#[program]
pub mod counter {
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
