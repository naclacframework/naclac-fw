use naclac_lang::prelude::*;

use crate::events::{CounterIncremented, NestedPadded, PaddedInner, SizedPayload};

/// Emits a plain zero-copy `#[event]` through `emit_cpi!`, and an
/// `#[event(alloc)]` one, covering both encodings the pinocchio backend uses.
#[event_cpi]
#[derive(Accounts)]
pub struct EventCpiEmit {}

pub fn event_cpi_emit(ctx: Context<EventCpiEmit>) -> Result {
    emit_cpi!(CounterIncremented { new_count: 7u64 });
    emit_cpi!(SizedPayload { data: Vec::from([1u8, 2, 3]) });
    Ok(())
}

pub fn event_cpi_emit_nested(ctx: Context<EventCpiEmit>) -> Result {
    let inner = PaddedInner {
        kind: 3,
        amount: 0x1122_3344_5566_7788,
    };
    let second = PaddedInner {
        kind: 9,
        amount: 42,
    };
    let event = NestedPadded {
        head: 5,
        inner,
        many: Vec::from([second, inner]),
        note: String::from("hi"),
    };
    emit_cpi!(event);
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
