//! Must FAIL: `#[event_cpi]` is in use but `Naclac.toml` has no entry for
//! this crate, so the event authority PDA cannot be derived at compile time.
use naclac_lang::prelude::*;

declare_id!("6queAeH2Cq6WPuq6tpgRFBtFBanSNW3zdc9Y8dm4g7oF");

#[event_cpi]
#[derive(Accounts)]
pub struct Emit {}

#[program]
pub mod event_cpi_missing_program_id {
    pub fn emit(_ctx: Context<Emit>) -> Result {
        Ok(())
    }
}
