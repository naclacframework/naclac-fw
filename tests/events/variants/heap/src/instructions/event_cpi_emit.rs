use naclac_lang::prelude::*;

use crate::events::CounterIncremented;

/// Emits `CounterIncremented` through `emit_cpi!` (borsh-encoded self-CPI).
#[event_cpi]
#[derive(Accounts)]
pub struct EventCpiEmit {}

pub fn event_cpi_emit(ctx: Context<EventCpiEmit>) -> Result {
    emit_cpi!(CounterIncremented { new_count: 7u64 });
    Ok(())
}

// The next three share `EventCpiEmit` (same account validation cost) and
// differ only in what they emit, so their CU difference is the emission.
pub fn event_cpi_noop(_ctx: Context<EventCpiEmit>) -> Result {
    Ok(())
}

pub fn event_cpi_log_one(_ctx: Context<EventCpiEmit>) -> Result {
    emit!(CounterIncremented { new_count: 7u64 });
    Ok(())
}

pub fn event_cpi_emit_one(ctx: Context<EventCpiEmit>) -> Result {
    emit_cpi!(CounterIncremented { new_count: 7u64 });
    Ok(())
}
