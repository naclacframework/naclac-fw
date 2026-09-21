use naclac_lang::prelude::*;

declare_id!("G2BmpsqrmJDX1QE8fKuaRYRZUmMCsF7gtZSAUzEFGCax");

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
