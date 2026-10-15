//! Must FAIL to build: `emit` uses the reserved event-CPI dispatch tag as
//! its discriminator while its accounts struct uses `#[event_cpi]`. Needs a
//! real crate (with a discoverable program ID in `Naclac.toml`) because
//! `#[event_cpi]`'s `event_authority` PDA is precomputed from that ID, which a
//! single-file trybuild fixture cannot provide.
use naclac_lang::prelude::*;

declare_id!("6queAeH2Cq6WPuq6tpgRFBtFBanSNW3zdc9Y8dm4g7oF");

#[event_cpi]
#[derive(Accounts)]
pub struct Emit {}

#[program]
pub mod event_cpi_tag_collision {
    #[discriminator = [34, 131, 189, 8, 189, 141, 224, 48]]
    pub fn emit(_ctx: Context<Emit>) -> Result {
        Ok(())
    }
}
