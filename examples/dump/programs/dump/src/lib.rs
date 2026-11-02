#![no_std]
use naclac_lang::prelude::*;

declare_id!("7YJD4TnKJAGgRmS332DnMgtsF4RW5HqvL9N6RwqX8KXg");

pub mod state;
pub mod instructions;
pub mod systems;
pub mod events;
pub mod errors;
pub mod constants;

use instructions::*;

#[program]
pub mod dump {
    pub fn initialize(ctx: Context<Initialize>) -> Result {
        initialize::initialize(ctx)
    }

    pub fn increment(ctx: Context<Increment>) -> Result {
        increment::increment(ctx)
    }
}
