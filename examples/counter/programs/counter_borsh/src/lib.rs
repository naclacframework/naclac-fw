use {
    instructions::*,
    naclac_lang::prelude::*,
};

declare_id!("BKPVSL9FUrPfdEzdiXQxyzgS46ktA1YYGVo52o3g2k1");

pub mod components;
pub mod instructions;
pub mod systems;
pub mod events;
pub mod errors;
pub mod constants;

#[program]
pub mod counter_borsh {
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
