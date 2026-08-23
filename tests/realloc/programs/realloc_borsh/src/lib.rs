use naclac_lang::prelude::*;

declare_id!("Djy4qEuibJRcS8baDaqT4xSrmgdVtq2bFgxoptFRts4u");

pub mod components;
pub mod constants;
pub mod instructions;

use instructions::*;

#[program]
pub mod realloc_prog {
    pub fn init_growable(ctx: Context<InitGrowable>) -> Result {
        init_growable::init_growable(ctx)
    }

    pub fn resize_growable(ctx: Context<ResizeGrowable>, new_space: u64) -> Result {
        resize_growable::resize_growable(ctx, new_space)
    }
}
