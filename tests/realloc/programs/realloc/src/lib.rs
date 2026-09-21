#![no_std]
use naclac_lang::prelude::*;

declare_id!("FSqCCvBJKjPaLAPcbehdb4oX9MunXidhspKpmGsJ41Nd");

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
