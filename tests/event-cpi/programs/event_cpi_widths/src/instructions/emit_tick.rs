use naclac_lang::prelude::*;

use crate::events::Ticked;

/// Shared by both `emit_a` and `emit_b`.
#[event_cpi]
#[derive(Accounts)]
pub struct EmitTick {}

pub fn emit_a(ctx: Context<EmitTick>) -> Result {
    emit_cpi!(Ticked { n: 11u64 });
    Ok(())
}

pub fn emit_b(ctx: Context<EmitTick>) -> Result {
    emit_cpi!(Ticked { n: 22u64 });
    Ok(())
}
