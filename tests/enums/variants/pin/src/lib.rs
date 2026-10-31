#![no_std]
use naclac_lang::prelude::*;

declare_id!("11111111111111111111111111111111");

pub mod instructions;
pub mod state;
pub mod types;

#[cfg(test)]
mod tests;

use instructions::*;
use types::{Action, Color, DynArgs, ListArgs, ModeArgs};

#[program]
pub mod enums {
    pub fn init_state(ctx: Context<InitState>) -> Result {
        instructions::init_state(ctx)
    }

    pub fn set_action(ctx: Context<Mutate>, action: Action) -> Result {
        instructions::set_action(ctx, action)
    }

    pub fn set_color(ctx: Context<Mutate>, color: Color) -> Result {
        instructions::set_color(ctx, color)
    }

    pub fn bench_noop(ctx: Context<Mutate>) -> Result {
        instructions::bench_noop(ctx)
    }

    pub fn bench_u8_set_const(ctx: Context<Mutate>) -> Result {
        instructions::bench_u8_set_const(ctx)
    }

    pub fn bench_color_set_const(ctx: Context<Mutate>) -> Result {
        instructions::bench_color_set_const(ctx)
    }

    pub fn bench_color_get(ctx: Context<Mutate>) -> Result {
        instructions::bench_color_get(ctx)
    }

    pub fn bench_action_set_const(ctx: Context<Mutate>) -> Result {
        instructions::bench_action_set_const(ctx)
    }

    pub fn bench_action_get(ctx: Context<Mutate>) -> Result {
        instructions::bench_action_get(ctx)
    }

    pub fn set_mode_with_args(ctx: Context<Mutate>, args: ModeArgs) -> Result {
        instructions::set_mode_with_args(ctx, args)
    }

    pub fn emit_state_changed(ctx: Context<Mutate>) -> Result {
        instructions::emit_state_changed(ctx)
    }

    pub fn emit_state_log(ctx: Context<Mutate>) -> Result {
        instructions::emit_state_log(ctx)
    }

    pub fn measure_cu(ctx: Context<Mutate>) -> Result {
        instructions::measure_cu(ctx)
    }

    pub fn dyn_args(ctx: Context<Mutate>, args: DynArgs, #[allow_heap] more: Vec<u32>) -> Result {
        instructions::dyn_args(ctx, args, more)
    }

    pub fn text_lists(
        ctx: Context<Mutate>,
        args: ListArgs,
        #[allow_heap] names: Vec<String>,
        #[allow_heap] blobs: Vec<Vec<u8>>,
    ) -> Result {
        instructions::text_lists(ctx, args, names, blobs)
    }

    pub fn enum_lists(
        ctx: Context<Mutate>,
        #[allow_heap] heap: Vec<Color>,
        view: ZcVec<ZcEnum<Color>>,
    ) -> Result {
        instructions::enum_lists(ctx, heap, view)
    }

    pub fn emit_mixed_event(ctx: Context<Mutate>) -> Result {
        instructions::emit_mixed_event(ctx)
    }
}
