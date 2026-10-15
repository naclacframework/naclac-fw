use naclac_lang::prelude::*;

use crate::events::{CounterIncremented, SizedPayload};

/// Emits a plain zero-copy `#[event]` through `emit_cpi!`, and an
/// `#[event(alloc)]` one, covering both encodings the zero-copy solana
/// backend uses.
#[event_cpi]
#[derive(Accounts)]
pub struct EventCpiEmit {}

pub fn event_cpi_emit(ctx: Context<EventCpiEmit>) -> Result {
    emit_cpi!(CounterIncremented { new_count: 7u64 });
    emit_cpi!(SizedPayload { data: Vec::from([1u8, 2, 3]) });
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
