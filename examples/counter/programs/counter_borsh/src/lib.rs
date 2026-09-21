use naclac_lang::prelude::*;

declare_id!("BYFM8wr8RjLM9RDjoAVaMw6YAqSZKxiTYHyggREpr4nS");

pub mod components;
pub mod instructions;
pub mod systems;
pub mod events;
pub mod errors;
pub mod constants;

use instructions::*;

#[program]
pub mod counter_borsh {
    pub fn initialize(ctx: Context<Initialize>) -> Result {
        initialize::initialize(ctx)
    }

    pub fn increment(ctx: Context<Increment>) -> Result {
        increment::increment(ctx)
    }
}
