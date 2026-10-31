#![no_std]
use naclac_lang::prelude::*;

declare_id!("11111111111111111111111111111111");

pub mod instructions;
pub mod state;

use instructions::*;

#[program]
pub mod fixed_capacity {
    pub fn init_profile(ctx: Context<InitProfile>) -> Result {
        instructions::init_profile(ctx)
    }

    pub fn push_score(ctx: Context<Mutate>, value: u64) -> Result {
        instructions::push_score(ctx, value)
    }

    pub fn pop_score(ctx: Context<Mutate>) -> Result {
        instructions::pop_score(ctx)
    }

    pub fn set_scores(ctx: Context<Mutate>, values: ZcVec<u64>) -> Result {
        instructions::set_scores(ctx, values)
    }

    pub fn set_name(ctx: Context<Mutate>, name: ZcString) -> Result {
        instructions::set_name(ctx, name)
    }

    pub fn add_member(ctx: Context<Mutate>, member: Address) -> Result {
        instructions::add_member(ctx, member)
    }

    pub fn remove_member(ctx: Context<Mutate>, index: u8) -> Result {
        instructions::remove_member(ctx, index)
    }

    pub fn push_flag(ctx: Context<Mutate>, value: u8) -> Result {
        instructions::push_flag(ctx, value)
    }

    pub fn bump_counter(ctx: Context<Mutate>) -> Result {
        instructions::bump_counter(ctx)
    }

    pub fn clear_all(ctx: Context<Mutate>) -> Result {
        instructions::clear_all(ctx)
    }
}
