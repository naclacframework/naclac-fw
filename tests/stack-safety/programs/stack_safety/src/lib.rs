use naclac_lang::prelude::*;

declare_id!("2NRNEsBPXjtD4GzyGyesA7N69KY9pZBo4cpj5dYjhu2K");

pub mod components;
pub mod constants;
pub mod instructions;

use instructions::*;

#[program]
pub mod stack_safety {
    pub fn init_small(ctx: Context<InitSmall>) -> Result {
        init_small::init_small(ctx)
    }

    pub fn touch_small(ctx: Context<TouchSmall>) -> Result {
        touch_small::touch_small(ctx)
    }

    pub fn init_big(ctx: Context<InitBig>) -> Result {
        init_big::init_big(ctx)
    }

    pub fn touch_big(ctx: Context<TouchBig>) -> Result {
        touch_big::touch_big(ctx)
    }

    pub fn close_big(ctx: Context<CloseBig>) -> Result {
        close_big::close_big(ctx)
    }
}
