//! Must SUCCEED: same shape as `event_cpi_tag_collision`, but `emit` keeps
//! its default (non-colliding) discriminator. Proves `#[event_cpi]` itself
//! compiles in a standalone crate, so the sibling fixture's failure is
//! attributable to the tag collision alone.
use naclac_lang::prelude::*;

declare_id!("6queAeH2Cq6WPuq6tpgRFBtFBanSNW3zdc9Y8dm4g7oF");

#[event_cpi]
#[derive(Accounts)]
pub struct Emit {}

#[program]
pub mod event_cpi_no_collision {
    pub fn emit(_ctx: Context<Emit>) -> Result {
        Ok(())
    }
}
