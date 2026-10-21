#![no_std]
use naclac_lang::prelude::*;

declare_id!("11111111111111111111111111111111");

pub mod events;
pub mod instructions;

use instructions::*;

// Instruction discriminators are 3 bytes and the event's is 5: the two
// categories' widths are independent, and the reserved event-CPI tag takes the
// instruction width. `emit_a` and `emit_b` share one `#[event_cpi]` accounts
// struct; `quiet` uses a struct without it.
#[program]
pub mod event_cpi_widths {
    #[discriminator = [1, 2, 3]]
    pub fn emit_a(ctx: Context<EmitTick>) -> Result {
        emit_tick::emit_a(ctx)
    }

    #[discriminator = [4, 5, 6]]
    pub fn emit_b(ctx: Context<EmitTick>) -> Result {
        emit_tick::emit_b(ctx)
    }

    #[discriminator = [7, 8, 9]]
    pub fn quiet(ctx: Context<Quiet>) -> Result {
        quiet::quiet(ctx)
    }
}
