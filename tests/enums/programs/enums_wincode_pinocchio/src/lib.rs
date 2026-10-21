use naclac_lang::prelude::*;

declare_id!("Euuix9YpS5aWiZAAuH14wdkiqTQC4aWGoni5eGuQTZ6w");

pub mod components;
pub mod instructions;
pub mod types;


use instructions::*;
use types::{Action, Color, ModeArgs};

#[program]
pub mod enums_wincode_pinocchio {
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
}
