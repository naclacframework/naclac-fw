use naclac_lang::prelude::*;

declare_id!("38kT2F7WBYF7bpZ8EUDqUSbMZXFH2pBQE3isMtHFKm4e");

pub mod components;
pub mod instructions;
pub mod systems;
pub mod events;
pub mod errors;
pub mod constants;

use instructions::*;

#[program]
pub mod counter_zc {
    pub fn initialize(ctx: Context<Initialize>) -> Result {
        initialize::initialize(ctx)
    }

    pub fn increment(ctx: Context<Increment>) -> Result {
        increment::increment(ctx)
    }
}
