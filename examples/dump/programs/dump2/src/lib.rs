use naclac_lang::prelude::*;

declare_id!("G7U8hK6QroSkXKenpY7dzgFXrUTApnaPNi6nfP2sF3Xw");

pub mod state;
pub mod instructions;
pub mod systems;
pub mod events;
pub mod errors;
pub mod constants;

use instructions::*;

#[program]
pub mod dump2 {
    pub fn initialize(ctx: Context<Initialize>) -> Result {
        initialize::initialize(ctx)
    }

    pub fn increment(ctx: Context<Increment>) -> Result {
        increment::increment(ctx)
    }
}
