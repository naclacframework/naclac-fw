#![no_std]
use naclac_lang::prelude::*;

declare_id!("11111111111111111111111111111111");

pub mod errors;
pub mod events;
pub mod instructions;
pub mod state;
pub mod types;

use {
    instructions::*,
    types::{Action, Color, Level, NestedArgs, OptionArgs, ScalarArgs, Wide},
};

#[program]
pub mod types_lab_borsh {
    #[discriminator = [1, 1]]
    pub fn init_lab(ctx: Context<InitLab>) -> Result {
        instructions::init_lab(ctx)
    }

    #[discriminator = [1, 2]]
    pub fn set_scalars(ctx: Context<Mutate>, args: ScalarArgs) -> Result {
        instructions::set_scalars(ctx, args)
    }

    #[discriminator = [1, 3]]
    pub fn set_color(ctx: Context<Mutate>, color: Color) -> Result {
        instructions::set_color(ctx, color)
    }

    #[discriminator = [1, 4]]
    pub fn set_action(ctx: Context<Mutate>, action: Action) -> Result {
        instructions::set_action(ctx, action)
    }

    #[discriminator = [1, 5]]
    pub fn set_level(ctx: Context<Mutate>, level: Level) -> Result {
        instructions::set_level(ctx, level)
    }

    #[discriminator = [1, 7]]
    pub fn set_wide(ctx: Context<Mutate>, wide: Wide) -> Result {
        instructions::set_wide(ctx, wide)
    }

    #[discriminator = [1, 8]]
    pub fn set_nested(ctx: Context<Mutate>, args: NestedArgs) -> Result {
        instructions::set_nested(ctx, args)
    }

    #[discriminator = [1, 9]]
    pub fn set_maybe_u64(ctx: Context<Mutate>, args: OptionArgs) -> Result {
        instructions::set_maybe_u64(ctx, args)
    }

    #[discriminator = [1, 10]]
    pub fn set_maybe_color(ctx: Context<Mutate>, color: Color) -> Result {
        instructions::set_maybe_color(ctx, color)
    }

    #[discriminator = [1, 11]]
    pub fn clear_maybe_color(ctx: Context<Mutate>) -> Result {
        instructions::clear_maybe_color(ctx)
    }

    #[discriminator = [1, 12]]
    pub fn set_text(ctx: Context<Mutate>, name: String) -> Result {
        instructions::set_text(ctx, name)
    }

    #[discriminator = [1, 13]]
    pub fn set_scores(ctx: Context<Mutate>, values: Vec<u64>) -> Result {
        instructions::set_scores(ctx, values)
    }

    #[discriminator = [1, 14]]
    pub fn push_score(ctx: Context<Mutate>, value: u64) -> Result {
        instructions::push_score(ctx, value)
    }

    #[discriminator = [1, 15]]
    pub fn pop_score(ctx: Context<Mutate>) -> Result {
        instructions::pop_score(ctx)
    }

    #[discriminator = [1, 16]]
    pub fn add_member(ctx: Context<Mutate>, member: Address) -> Result {
        instructions::add_member(ctx, member)
    }

    #[discriminator = [1, 17]]
    pub fn remove_member(ctx: Context<Mutate>, index: u8) -> Result {
        instructions::remove_member(ctx, index)
    }

    #[discriminator = [1, 18]]
    pub fn push_flag(ctx: Context<Mutate>, value: u8) -> Result {
        instructions::push_flag(ctx, value)
    }

    #[discriminator = [1, 19]]
    pub fn clear_roster(ctx: Context<Mutate>) -> Result {
        instructions::clear_roster(ctx)
    }

    #[discriminator = [1, 20]]
    pub fn set_roster(ctx: Context<Mutate>, scores: Vec<u64>, name: String) -> Result {
        instructions::set_roster(ctx, scores, name)
    }

    #[discriminator = [1, 21]]
    pub fn bump_counter(ctx: Context<Mutate>) -> Result {
        instructions::bump_counter(ctx)
    }

    #[discriminator = [1, 22]]
    pub fn fail_with(ctx: Context<Mutate>, code: u8) -> Result {
        instructions::fail_with(ctx, code)
    }

    #[discriminator = [1, 23]]
    pub fn emit_log(ctx: Context<Mutate>) -> Result {
        instructions::emit_log(ctx)
    }

    #[discriminator = [1, 24]]
    pub fn emit_cpi_log(ctx: Context<EmitCpiLog>) -> Result {
        instructions::emit_cpi_log(ctx)
    }

    #[discriminator = [1, 25]]
    pub fn init_note(ctx: Context<InitNote>) -> Result {
        instructions::init_note(ctx)
    }

    #[discriminator = [1, 26]]
    pub fn touch_optional(ctx: Context<TouchOptional>) -> Result {
        instructions::touch_optional(ctx)
    }
}
