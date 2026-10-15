//! Must FAIL to build: the handler calls `emit_cpi!` but its accounts struct
//! (`Plain`) has no `#[event_cpi]`, so it has no `EventCpiAuthority` impl.
//! Checked by `tests/diagnostics.rs` (the error's note points into naclac-core
//! with an absolute path, so it can't be a `trybuild` snapshot).
use naclac_lang::prelude::*;

declare_id!("6queAeH2Cq6WPuq6tpgRFBtFBanSNW3zdc9Y8dm4g7oF");

#[event]
pub struct Ticked {
    pub n: u64,
}

#[derive(Accounts)]
pub struct Plain {}

#[program]
pub mod emit_cpi_without_event_cpi {
    pub fn tick(ctx: Context<Plain>) -> Result {
        emit_cpi!(Ticked { n: 1u64 });
        Ok(())
    }
}
