use naclac_lang::prelude::*;

declare_id!("4T82TMaMnPbKhbeTBMfMrkujQbWabDbNXRLfwmbnskoR");

pub mod events;
pub mod instructions;

use instructions::*;

// Instruction discriminators are 2 bytes and the event's is 7: the two
// categories' widths are independent, and the reserved event-CPI tag takes the
// instruction width. `emit_a` and `emit_b` share one `#[event_cpi]` accounts
// struct; `quiet` uses a struct without it.
#[program]
pub mod event_cpi_widths_zc {
    #[discriminator = [1, 2]]
    pub fn emit_a(ctx: Context<EmitTick>) -> Result {
        emit_tick::emit_a(ctx)
    }

    #[discriminator = [3, 4]]
    pub fn emit_b(ctx: Context<EmitTick>) -> Result {
        emit_tick::emit_b(ctx)
    }

    #[discriminator = [5, 6]]
    pub fn quiet(ctx: Context<Quiet>) -> Result {
        quiet::quiet(ctx)
    }
}
